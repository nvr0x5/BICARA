//! Meeting history in SQLite (`<app data>/bicara.db`), with FTS5 full-text search over
//! transcripts, notes and context-document chunks.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

pub struct Store {
    db: Connection,
}

#[derive(Debug, Clone, Serialize)]
pub struct Meeting {
    pub id: i64,
    pub title: String,
    pub preset_id: String,
    pub app_name: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub notulen: Option<String>,
    pub recording_dir: Option<String>,
    pub utterances: i64,
    pub documents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub id: i64,
    pub meeting_id: i64,
    pub name: String,
    pub chars: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredUtterance {
    pub source: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredAnswer {
    pub id: i64,
    pub action: String,
    pub title: String,
    pub input: Option<String>,
    pub text: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub meeting_id: i64,
    pub title: String,
    pub started_at: i64,
    pub snippet: String,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS meetings (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  preset_id TEXT NOT NULL,
  app_name TEXT,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  notulen TEXT,
  recording_dir TEXT
);
CREATE TABLE IF NOT EXISTS utterances (
  id INTEGER PRIMARY KEY,
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  source TEXT NOT NULL,
  text TEXT NOT NULL,
  start REAL NOT NULL,
  end REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS answers (
  id INTEGER PRIMARY KEY,
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  action TEXT NOT NULL,
  title TEXT NOT NULL,
  input TEXT,
  text TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS documents (
  id INTEGER PRIMARY KEY,
  meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  text TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chunks (
  id INTEGER PRIMARY KEY,
  document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  meeting_id INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  text TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(text, content='chunks', content_rowid='id', tokenize='unicode61 remove_diacritics 2');
CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
  INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE VIRTUAL TABLE IF NOT EXISTS utterances_fts USING fts5(text, content='utterances', content_rowid='id', tokenize='unicode61 remove_diacritics 2');
CREATE TRIGGER IF NOT EXISTS utterances_ai AFTER INSERT ON utterances BEGIN
  INSERT INTO utterances_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER IF NOT EXISTS utterances_ad AFTER DELETE ON utterances BEGIN
  INSERT INTO utterances_fts(utterances_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE INDEX IF NOT EXISTS utterances_meeting ON utterances(meeting_id);
CREATE INDEX IF NOT EXISTS chunks_meeting ON chunks(meeting_id);
"#;

/// Common Indonesian/English words that make poor search terms.
const STOPWORDS: &[&str] = &[
    "yang",
    "dan",
    "di",
    "ke",
    "dari",
    "ini",
    "itu",
    "untuk",
    "dengan",
    "adalah",
    "ada",
    "akan",
    "pada",
    "juga",
    "tidak",
    "saya",
    "anda",
    "kami",
    "kita",
    "mereka",
    "bisa",
    "apa",
    "apakah",
    "bagaimana",
    "gimana",
    "kenapa",
    "mengapa",
    "tentang",
    "sudah",
    "belum",
    "atau",
    "karena",
    "jadi",
    "kalau",
    "seperti",
    "lebih",
    "tolong",
    "jelaskan",
    "ceritakan",
    "boleh",
    "the",
    "and",
    "for",
    "you",
    "your",
    "what",
    "how",
    "why",
    "with",
    "this",
    "that",
    "are",
    "can",
];

/// Turn free text into a safe FTS5 OR-query of meaningful words.
pub fn fts_query(text: &str) -> Option<String> {
    let mut words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3 && !STOPWORDS.contains(w))
        .map(|w| format!("\"{w}\""))
        .collect();
    words.dedup();
    words.truncate(24);
    (!words.is_empty()).then(|| words.join(" OR "))
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.execute_batch(SCHEMA)?;
        Ok(Self { db })
    }

    #[cfg(test)]
    pub fn memory() -> Self {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        Self { db }
    }

    pub fn create_meeting(&self, title: &str, preset_id: &str) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO meetings(title, preset_id, started_at) VALUES (?1, ?2, ?3)",
            params![title, preset_id, now()],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn update_meeting_start(
        &self,
        id: i64,
        app_name: Option<&str>,
        recording_dir: &str,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE meetings SET app_name = ?2, recording_dir = ?3, started_at = ?4 WHERE id = ?1",
            params![id, app_name, recording_dir, now()],
        )?;
        Ok(())
    }

    pub fn end_meeting(&self, id: i64) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE meetings SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
            params![id, now()],
        )?;
        Ok(())
    }

    pub fn set_notulen(&self, id: i64, text: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE meetings SET notulen = ?2 WHERE id = ?1",
            params![id, text],
        )?;
        Ok(())
    }

    pub fn rename_meeting(&self, id: i64, title: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "UPDATE meetings SET title = ?2 WHERE id = ?1",
            params![id, title],
        )?;
        Ok(())
    }

    pub fn delete_meeting(&self, id: i64) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM meetings WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn meeting_row(r: &rusqlite::Row) -> rusqlite::Result<Meeting> {
        Ok(Meeting {
            id: r.get(0)?,
            title: r.get(1)?,
            preset_id: r.get(2)?,
            app_name: r.get(3)?,
            started_at: r.get(4)?,
            ended_at: r.get(5)?,
            notulen: r.get(6)?,
            recording_dir: r.get(7)?,
            utterances: r.get(8)?,
            documents: r.get(9)?,
        })
    }

    const MEETING_COLS: &'static str = "m.id, m.title, m.preset_id, m.app_name, m.started_at, m.ended_at, m.notulen, m.recording_dir, \
        (SELECT COUNT(*) FROM utterances u WHERE u.meeting_id = m.id), \
        (SELECT COUNT(*) FROM documents d WHERE d.meeting_id = m.id)";

    pub fn meeting(&self, id: i64) -> rusqlite::Result<Option<Meeting>> {
        self.db
            .query_row(
                &format!(
                    "SELECT {} FROM meetings m WHERE m.id = ?1",
                    Self::MEETING_COLS
                ),
                [id],
                Self::meeting_row,
            )
            .optional()
    }

    pub fn meetings(&self, limit: i64) -> rusqlite::Result<Vec<Meeting>> {
        let mut st = self.db.prepare(&format!(
            "SELECT {} FROM meetings m ORDER BY m.started_at DESC, m.id DESC LIMIT ?1",
            Self::MEETING_COLS
        ))?;
        let rows = st.query_map([limit], Self::meeting_row)?;
        rows.collect()
    }

    pub fn add_utterance(
        &self,
        meeting_id: i64,
        source: &str,
        text: &str,
        start: f64,
        end: f64,
    ) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT INTO utterances(meeting_id, source, text, start, end) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![meeting_id, source, text, start, end],
        )?;
        Ok(())
    }

    pub fn utterances(&self, meeting_id: i64) -> rusqlite::Result<Vec<StoredUtterance>> {
        let mut st = self.db.prepare(
            "SELECT source, text, start, end FROM utterances WHERE meeting_id = ?1 ORDER BY id",
        )?;
        let rows = st.query_map([meeting_id], |r| {
            Ok(StoredUtterance {
                source: r.get(0)?,
                text: r.get(1)?,
                start: r.get(2)?,
                end: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_answer(
        &self,
        meeting_id: i64,
        action: &str,
        title: &str,
        input: Option<&str>,
        text: &str,
    ) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT INTO answers(meeting_id, action, title, input, text, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![meeting_id, action, title, input, text, now()],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    pub fn answers(&self, meeting_id: i64) -> rusqlite::Result<Vec<StoredAnswer>> {
        let mut st = self
            .db
            .prepare("SELECT id, action, title, input, text, created_at FROM answers WHERE meeting_id = ?1 ORDER BY id")?;
        let rows = st.query_map([meeting_id], |r| {
            Ok(StoredAnswer {
                id: r.get(0)?,
                action: r.get(1)?,
                title: r.get(2)?,
                input: r.get(3)?,
                text: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_document(
        &self,
        meeting_id: i64,
        name: &str,
        text: &str,
    ) -> rusqlite::Result<Document> {
        let tx = self.db.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO documents(meeting_id, name, text) VALUES (?1, ?2, ?3)",
            params![meeting_id, name, text],
        )?;
        let doc_id = tx.last_insert_rowid();
        for (i, c) in bicara_docs::chunk(text, 900, 150).iter().enumerate() {
            tx.execute(
                "INSERT INTO chunks(document_id, meeting_id, idx, text) VALUES (?1, ?2, ?3, ?4)",
                params![doc_id, meeting_id, i as i64, c],
            )?;
        }
        tx.commit()?;
        Ok(Document {
            id: doc_id,
            meeting_id,
            name: name.into(),
            chars: text.chars().count() as i64,
        })
    }

    pub fn remove_document(&self, doc_id: i64) -> rusqlite::Result<()> {
        self.db
            .execute("DELETE FROM documents WHERE id = ?1", [doc_id])?;
        Ok(())
    }

    pub fn documents(&self, meeting_id: i64) -> rusqlite::Result<Vec<Document>> {
        let mut st = self
            .db
            .prepare("SELECT id, meeting_id, name, length(text) FROM documents WHERE meeting_id = ?1 ORDER BY id")?;
        let rows = st.query_map([meeting_id], |r| {
            Ok(Document {
                id: r.get(0)?,
                meeting_id: r.get(1)?,
                name: r.get(2)?,
                chars: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Context documents for a prompt. Small sets are included whole (most reliable);
    /// large sets are narrowed to the chunks that best match `query` (BM25).
    pub fn document_context(
        &self,
        meeting_id: i64,
        query: &str,
        budget_chars: usize,
    ) -> rusqlite::Result<String> {
        let mut st = self
            .db
            .prepare("SELECT name, text FROM documents WHERE meeting_id = ?1 ORDER BY id")?;
        let docs: Vec<(String, String)> = st
            .query_map([meeting_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if docs.is_empty() {
            return Ok(String::new());
        }
        let total: usize = docs.iter().map(|(_, t)| t.chars().count()).sum();
        if total <= budget_chars {
            return Ok(docs
                .iter()
                .map(|(n, t)| format!("<dokumen nama=\"{n}\">\n{t}\n</dokumen>"))
                .collect::<Vec<_>>()
                .join("\n\n"));
        }
        let mut picked: Vec<(String, i64, String)> = Vec::new();
        if let Some(q) = fts_query(query) {
            let mut st = self.db.prepare(
                "SELECT d.name, c.idx, c.text FROM chunks_fts f JOIN chunks c ON c.id = f.rowid \
                 JOIN documents d ON d.id = c.document_id \
                 WHERE chunks_fts MATCH ?1 AND c.meeting_id = ?2 ORDER BY bm25(chunks_fts) LIMIT 12",
            )?;
            picked = st
                .query_map(params![q, meeting_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
        }
        if picked.is_empty() {
            // Nothing matched: give the opening of each document.
            let mut st = self.db.prepare(
                "SELECT d.name, c.idx, c.text FROM chunks c JOIN documents d ON d.id = c.document_id \
                 WHERE c.meeting_id = ?1 AND c.idx < 2 ORDER BY d.id, c.idx",
            )?;
            picked = st
                .query_map([meeting_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?;
        }
        let mut out = String::new();
        for (name, idx, text) in picked {
            let block = format!(
                "<kutipan dokumen=\"{name}\" bagian=\"{}\">\n{text}\n</kutipan>\n\n",
                idx + 1
            );
            if out.chars().count() + block.chars().count() > budget_chars {
                break;
            }
            out.push_str(&block);
        }
        Ok(out)
    }

    /// Search past meetings by title, notes and transcript text.
    pub fn search(&self, query: &str) -> rusqlite::Result<Vec<SearchHit>> {
        let Some(q) = fts_query(query) else {
            return Ok(vec![]);
        };
        let mut st = self.db.prepare(
            "SELECT m.id, m.title, m.started_at, snippet(utterances_fts, 0, '«', '»', '…', 12) \
             FROM utterances_fts f JOIN utterances u ON u.id = f.rowid JOIN meetings m ON m.id = u.meeting_id \
             WHERE utterances_fts MATCH ?1 ORDER BY bm25(utterances_fts) LIMIT 40",
        )?;
        let mut hits: Vec<SearchHit> = st
            .query_map([&q], |r| {
                Ok(SearchHit {
                    meeting_id: r.get(0)?,
                    title: r.get(1)?,
                    started_at: r.get(2)?,
                    snippet: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        // Title / notulen matches (plain LIKE; few rows).
        let like = format!("%{}%", query.trim());
        let mut st = self.db.prepare(
            "SELECT id, title, started_at, substr(coalesce(notulen, ''), 1, 160) FROM meetings \
             WHERE title LIKE ?1 OR notulen LIKE ?1 ORDER BY started_at DESC LIMIT 20",
        )?;
        let extra: Vec<SearchHit> = st
            .query_map([&like], |r| {
                Ok(SearchHit {
                    meeting_id: r.get(0)?,
                    title: r.get(1)?,
                    started_at: r.get(2)?,
                    snippet: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        hits.extend(extra);
        let mut seen = std::collections::HashSet::new();
        hits.retain(|h| seen.insert(h.meeting_id));
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meeting_lifecycle_and_search() {
        let s = Store::memory();
        let id = s.create_meeting("Rapat OKR Q4", "rapat").unwrap();
        s.add_utterance(
            id,
            "app",
            "Target pertumbuhan pengguna kuartal depan dua puluh persen.",
            1.0,
            4.0,
        )
        .unwrap();
        s.add_utterance(
            id,
            "mic",
            "Setuju, tapi anggaran marketing perlu ditambah.",
            5.0,
            8.0,
        )
        .unwrap();
        s.end_meeting(id).unwrap();
        s.set_notulen(id, "**Keputusan**: target 20%").unwrap();
        let m = s.meeting(id).unwrap().unwrap();
        assert_eq!(m.utterances, 2);
        assert!(m.ended_at.is_some());
        assert_eq!(s.search("anggaran marketing").unwrap()[0].meeting_id, id);
        assert_eq!(s.search("OKR").unwrap()[0].meeting_id, id);
        assert!(s.search("yang dan di").unwrap().is_empty());
        s.delete_meeting(id).unwrap();
        assert!(s.meetings(10).unwrap().is_empty());
    }

    #[test]
    fn document_context_small_and_large() {
        let s = Store::memory();
        let id = s.create_meeting("Interview", "interview").unwrap();
        s.add_document(
            id,
            "cv.txt",
            "Budi. Product Manager di Tokopedia. Memimpin tim 8 orang.",
        )
        .unwrap();
        let small = s.document_context(id, "pengalaman", 10_000).unwrap();
        assert!(small.contains("<dokumen nama=\"cv.txt\">") && small.contains("Tokopedia"));

        let filler = "Paragraf umum tentang kebijakan kantor dan jam kerja. ".repeat(400);
        s.add_document(
            id,
            "handbook.txt",
            &format!("{filler}\n\nKebijakan cuti melahirkan adalah tiga bulan penuh.\n\n{filler}"),
        )
        .unwrap();
        let large = s
            .document_context(id, "Berapa lama cuti melahirkan?", 3_000)
            .unwrap();
        assert!(
            large.contains("cuti melahirkan adalah tiga bulan"),
            "{large}"
        );
        assert!(large.chars().count() <= 3_000);
    }

    #[test]
    fn fts_query_is_safe() {
        assert_eq!(
            fts_query("Apa itu \"OKR\"? (AND) OR"),
            Some("\"okr\"".into())
        );
        assert_eq!(fts_query("yang di ke"), None);
    }
}
