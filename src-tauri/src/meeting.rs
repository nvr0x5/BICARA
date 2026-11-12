//! Meeting lifecycle: draft (title, preset, context files) → Mulai meeting (capture + live AI +
//! recording) → Selesai (stop, automatic notulen) → history, export, share, ask.

use std::path::PathBuf;

use bicara_llm::ChatRequest;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;
use crate::assist::{self, Action};
use crate::store::{Document, Meeting, SearchHit, StoredAnswer, StoredUtterance};

fn e(x: impl std::fmt::Display) -> String {
    x.to_string()
}

pub fn fmt_date(ts: i64) -> String {
    use chrono::{Datelike, Local, TimeZone, Timelike};
    const BULAN: [&str; 12] = [
        "Januari",
        "Februari",
        "Maret",
        "April",
        "Mei",
        "Juni",
        "Juli",
        "Agustus",
        "September",
        "Oktober",
        "November",
        "Desember",
    ];
    match Local.timestamp_opt(ts, 0).single() {
        Some(d) => format!(
            "{} {} {}, {:02}.{:02}",
            d.day(),
            BULAN[d.month0() as usize],
            d.year(),
            d.hour(),
            d.minute()
        ),
        None => String::new(),
    }
}

// ---------------------------------------------------------------- draft & files

#[tauri::command]
pub fn create_meeting(
    state: State<'_, AppState>,
    title: String,
    preset_id: String,
) -> Result<Meeting, String> {
    let title = if title.trim().is_empty() {
        "Meeting tanpa judul".to_string()
    } else {
        title.trim().to_string()
    };
    let store = state.store.lock().unwrap();
    let id = store.create_meeting(&title, &preset_id).map_err(e)?;
    store
        .meeting(id)
        .map_err(e)?
        .ok_or_else(|| "meeting hilang".into())
}

#[derive(Serialize)]
pub struct FileResult {
    name: String,
    document: Option<Document>,
    error: Option<String>,
}

/// Read and index context files (PDF, DOCX, PPTX, TXT, MD) for a meeting.
#[tauri::command(async)]
pub fn add_meeting_files(
    state: State<'_, AppState>,
    meeting_id: i64,
    paths: Vec<String>,
) -> Vec<FileResult> {
    paths
        .into_iter()
        .map(|p| {
            let path = PathBuf::from(&p);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(p.clone());
            match bicara_docs::extract(&path) {
                Ok(text) => match state
                    .store
                    .lock()
                    .unwrap()
                    .add_document(meeting_id, &name, &text)
                {
                    Ok(d) => FileResult {
                        name,
                        document: Some(d),
                        error: None,
                    },
                    Err(err) => FileResult {
                        name,
                        document: None,
                        error: Some(err.to_string()),
                    },
                },
                Err(err) => FileResult {
                    name,
                    document: None,
                    error: Some(err.to_string()),
                },
            }
        })
        .collect()
}

#[tauri::command]
pub fn remove_meeting_document(state: State<'_, AppState>, document_id: i64) -> Result<(), String> {
    state
        .store
        .lock()
        .unwrap()
        .remove_document(document_id)
        .map_err(e)
}

#[tauri::command]
pub fn supported_document_extensions() -> Vec<&'static str> {
    bicara_docs::supported_extensions().to_vec()
}

// ---------------------------------------------------------------- start / end

#[derive(Serialize)]
pub struct Started {
    meeting: Meeting,
    transcribing: bool,
    warning: Option<String>,
}

#[tauri::command(async)]
pub fn start_meeting(
    app: AppHandle,
    meeting_id: i64,
    app_id: Option<String>,
    app_name: Option<String>,
    mic: bool,
) -> Result<Started, String> {
    let state = app.state::<AppState>();
    if state.current_meeting.lock().unwrap().is_some() {
        return Err("Masih ada meeting yang berjalan. Selesaikan dulu.".into());
    }
    // Use the meeting's preset for the whole session.
    {
        let store = state.store.lock().unwrap();
        let m = store
            .meeting(meeting_id)
            .map_err(e)?
            .ok_or("meeting tidak ditemukan")?;
        state.settings.lock().unwrap().active_preset = m.preset_id;
    }
    assist::prewarm(&app);
    state.transcript.lock().unwrap().clear();
    *state.last_question.lock().unwrap() = None;

    let info = crate::capture::start_capture(app.clone(), app.state(), app_id, mic)?;
    let store = state.store.lock().unwrap();
    store
        .update_meeting_start(meeting_id, app_name.as_deref(), &info.dir)
        .map_err(e)?;
    *state.current_meeting.lock().unwrap() = Some(meeting_id);
    let meeting = store
        .meeting(meeting_id)
        .map_err(e)?
        .ok_or("meeting tidak ditemukan")?;
    let _ = app.emit("meeting-state", &meeting);
    Ok(Started {
        meeting,
        transcribing: info.transcribing,
        warning: info.warning,
    })
}

/// Stop capture, close the meeting and generate the notulen in the background.
#[tauri::command(async)]
pub fn end_meeting(app: AppHandle) -> Result<Option<i64>, String> {
    let state = app.state::<AppState>();
    let _ = crate::capture::stop_capture(app.state());
    // Give the speech model a moment to finish the last utterance(s).
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let Some(mid) = state.current_meeting.lock().unwrap().take() else {
        return Ok(None);
    };
    state.store.lock().unwrap().end_meeting(mid).map_err(e)?;
    if !state.settings.lock().unwrap().keep_audio {
        if let Ok(Some(dir)) = state
            .store
            .lock()
            .unwrap()
            .meeting(mid)
            .map(|m| m.and_then(|m| m.recording_dir))
        {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    let _ = app.emit("meeting-state", Option::<Meeting>::None);
    generate_notulen(app.clone(), mid)?;
    Ok(Some(mid))
}

#[tauri::command]
pub fn current_meeting(state: State<'_, AppState>) -> Option<Meeting> {
    let id = (*state.current_meeting.lock().unwrap())?;
    state.store.lock().unwrap().meeting(id).ok().flatten()
}

// ---------------------------------------------------------------- notulen

fn stored_transcript(lines: &[StoredUtterance], max: usize) -> String {
    let us: Vec<bicara_stt::Utterance> = lines
        .iter()
        .map(|u| bicara_stt::Utterance {
            source: u.source.clone(),
            text: u.text.clone(),
            start: u.start,
            end: u.end,
            latency_ms: 0,
            partial: false,
        })
        .collect();
    assist::transcript_text(&us, max)
}

#[tauri::command]
pub fn generate_notulen(app: AppHandle, meeting_id: i64) -> Result<u64, String> {
    let state = app.state::<AppState>();
    let (m, lines) = {
        let store = state.store.lock().unwrap();
        (
            store
                .meeting(meeting_id)
                .map_err(e)?
                .ok_or("meeting tidak ditemukan")?,
            store.utterances(meeting_id).map_err(e)?,
        )
    };
    if lines.is_empty() {
        let msg = "Tidak ada transkrip, jadi notulen tidak dibuat.";
        state
            .store
            .lock()
            .unwrap()
            .set_notulen(meeting_id, msg)
            .map_err(e)?;
        let _ = app.emit("notulen-ready", meeting_id);
        return Ok(0);
    }
    let transcript = stored_transcript(&lines, 90_000);
    let docs = assist::documents_block(&app, Some(meeting_id), &transcript, 12_000);
    let settings = state.settings.lock().unwrap().clone();
    let duration = m
        .ended_at
        .map(|end| (end - m.started_at).max(0) / 60)
        .unwrap_or(0);
    let user = format!(
        "{docs}<transkrip>\n{transcript}\n</transkrip>\n\n\
         Buat NOTULEN rapat dalam bahasa Indonesia untuk meeting berjudul «{}» ({}, durasi ±{duration} menit).\n\
         Gunakan format markdown persis seperti ini (lewati bagian yang benar-benar kosong):\n\n\
         # {}\n\
         **Tanggal:** …\n\
         **Peserta:** (sebut nama/peran yang terdengar; \"Saya\" adalah pengguna)\n\n\
         ## Ringkasan\n- 3–6 poin paling penting\n\n\
         ## Poin Diskusi\n- topik → inti pembahasannya\n\n\
         ## Keputusan\n- …\n\n\
         ## Action Items\n- Tugas — PIC: nama — Tenggat: tanggal (tulis \"?\" jika tidak disebut)\n\n\
         ## Pertanyaan Terbuka\n- …\n\n\
         Hanya tulis hal yang benar-benar ada di transkrip. Jangan mengarang nama, angka, atau tanggal.",
        m.title,
        fmt_date(m.started_at),
        m.title
    );
    let req = ChatRequest {
        system: assist::system_prompt(&settings),
        user,
        max_tokens: 3000,
    };
    let on_done: assist::OnDone = Box::new(move |app: &AppHandle, text: &str| {
        let st = app.state::<AppState>();
        let _ = st.store.lock().unwrap().set_notulen(meeting_id, text);
        let _ = app.emit("notulen-ready", meeting_id);
    });
    Ok(assist::stream_llm(
        app.clone(),
        None,
        Action::Notulen,
        format!("Notulen · {}", m.title),
        None,
        req,
        Some(on_done),
    ))
}

#[tauri::command]
pub fn save_notulen(
    state: State<'_, AppState>,
    meeting_id: i64,
    text: String,
) -> Result<(), String> {
    state
        .store
        .lock()
        .unwrap()
        .set_notulen(meeting_id, &text)
        .map_err(e)
}

// ---------------------------------------------------------------- interview prep

const HRD_QUESTIONS: &[&str] = &[
    "Ceritakan tentang diri Anda.",
    "Kenapa Anda tertarik dengan posisi dan perusahaan ini?",
    "Apa kelebihan dan kekurangan Anda?",
    "Ceritakan pencapaian terbesar Anda.",
    "Ceritakan saat Anda menghadapi konflik di tim dan cara menyelesaikannya.",
    "Ceritakan kegagalan yang pernah Anda alami dan apa pelajarannya.",
    "Bagaimana Anda mengatur prioritas saat banyak tenggat bersamaan?",
    "Kenapa Anda ingin pindah dari pekerjaan sekarang?",
    "Di mana Anda melihat diri Anda 5 tahun lagi?",
    "Berapa ekspektasi gaji Anda?",
    "Kapan Anda bisa mulai bekerja?",
    "Ada pertanyaan untuk kami?",
];

#[tauri::command]
pub fn interview_questions() -> Vec<&'static str> {
    HRD_QUESTIONS.to_vec()
}

/// Prepare STAR answers to common Indonesian HRD questions from the meeting's files (CV, JD).
#[tauri::command]
pub fn prepare_interview(app: AppHandle, meeting_id: i64) -> Result<u64, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let docs = assist::documents_block(
        &app,
        Some(meeting_id),
        "pengalaman proyek pencapaian posisi kualifikasi",
        30_000,
    );
    if docs.is_empty() && settings.preset().context.trim().is_empty() {
        return Err(
            "Unggah CV dan/atau deskripsi pekerjaan dulu supaya jawabannya personal.".into(),
        );
    }
    let list = HRD_QUESTIONS
        .iter()
        .enumerate()
        .map(|(i, q)| format!("{}. {q}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "{docs}Saya akan diwawancara. Siapkan jawaban untuk pertanyaan HRD yang umum di Indonesia berikut, \
         berdasarkan CV/pengalaman dan deskripsi pekerjaan di dokumen:\n{list}\n\n\
         Untuk setiap pertanyaan tulis: judul pertanyaan dalam **tebal**, lalu jawaban yang natural untuk \
         diucapkan (3–5 kalimat). Untuk pertanyaan perilaku gunakan STAR (Situasi, Tugas, Aksi, Hasil) dengan \
         contoh nyata dari dokumen. Jangan mengarang pengalaman; jika data tidak ada, beri kerangka jawaban \
         dengan [isi sendiri]. Untuk gaji, beri strategi menjawab, bukan angka."
    );
    let req = ChatRequest {
        system: assist::system_prompt(&settings),
        user,
        max_tokens: 4000,
    };
    Ok(assist::stream_llm(
        app.clone(),
        Some(meeting_id),
        Action::Prep,
        "Persiapan interview".into(),
        None,
        req,
        None,
    ))
}

// ---------------------------------------------------------------- history

#[derive(Serialize)]
pub struct MeetingDetail {
    meeting: Meeting,
    date: String,
    utterances: Vec<StoredUtterance>,
    answers: Vec<StoredAnswer>,
    documents: Vec<Document>,
}

#[tauri::command]
pub fn list_meetings(state: State<'_, AppState>) -> Result<Vec<Meeting>, String> {
    state.store.lock().unwrap().meetings(500).map_err(e)
}

#[tauri::command]
pub fn search_meetings(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchHit>, String> {
    state.store.lock().unwrap().search(&query).map_err(e)
}

#[tauri::command]
pub fn get_meeting(state: State<'_, AppState>, meeting_id: i64) -> Result<MeetingDetail, String> {
    let store = state.store.lock().unwrap();
    let meeting = store
        .meeting(meeting_id)
        .map_err(e)?
        .ok_or("meeting tidak ditemukan")?;
    Ok(MeetingDetail {
        date: fmt_date(meeting.started_at),
        utterances: store.utterances(meeting_id).map_err(e)?,
        answers: store.answers(meeting_id).map_err(e)?,
        documents: store.documents(meeting_id).map_err(e)?,
        meeting,
    })
}

#[tauri::command]
pub fn rename_meeting(
    state: State<'_, AppState>,
    meeting_id: i64,
    title: String,
) -> Result<(), String> {
    state
        .store
        .lock()
        .unwrap()
        .rename_meeting(meeting_id, title.trim())
        .map_err(e)
}

/// Deletes the meeting (transcript, notes, files) and its audio recordings.
#[tauri::command]
pub fn delete_meeting(state: State<'_, AppState>, meeting_id: i64) -> Result<(), String> {
    if *state.current_meeting.lock().unwrap() == Some(meeting_id) {
        return Err("Meeting ini sedang berjalan.".into());
    }
    let store = state.store.lock().unwrap();
    if let Some(dir) = store
        .meeting(meeting_id)
        .map_err(e)?
        .and_then(|m| m.recording_dir)
    {
        let _ = std::fs::remove_dir_all(dir);
    }
    store.delete_meeting(meeting_id).map_err(e)
}

/// "Tanya meeting lalu": answer a question from a past meeting's notes, transcript and files.
#[tauri::command]
pub fn ask_meeting(app: AppHandle, meeting_id: i64, question: String) -> Result<u64, String> {
    let state = app.state::<AppState>();
    let (m, lines) = {
        let store = state.store.lock().unwrap();
        (
            store
                .meeting(meeting_id)
                .map_err(e)?
                .ok_or("meeting tidak ditemukan")?,
            store.utterances(meeting_id).map_err(e)?,
        )
    };
    let transcript = stored_transcript(&lines, 80_000);
    let docs = assist::documents_block(&app, Some(meeting_id), &question, 12_000);
    let settings = state.settings.lock().unwrap().clone();
    let user = format!(
        "{docs}Meeting «{}» pada {}.\n<notulen>\n{}\n</notulen>\n<transkrip>\n{transcript}\n</transkrip>\n\n\
         Jawab pertanyaan Saya tentang meeting ini berdasarkan notulen, transkrip, dan dokumen di atas. \
         Jika jawabannya tidak ada, katakan begitu.\n\nPertanyaan: {question}",
        m.title,
        fmt_date(m.started_at),
        m.notulen.as_deref().unwrap_or("(belum ada)")
    );
    let req = ChatRequest {
        system: assist::system_prompt(&settings),
        user,
        max_tokens: 1200,
    };
    Ok(assist::stream_llm(
        app.clone(),
        Some(meeting_id),
        Action::Ask,
        "Tanya meeting".into(),
        Some(question),
        req,
        None,
    ))
}

// ---------------------------------------------------------------- export & share

fn export_markdown(d: &MeetingDetail) -> String {
    let mut out = d
        .meeting
        .notulen
        .clone()
        .unwrap_or_else(|| format!("# {}\n\n_Belum ada notulen._", d.meeting.title));
    out.push_str("\n\n---\n\n## Transkrip\n\n");
    for u in &d.utterances {
        let s = u.start as u64;
        out.push_str(&format!(
            "**[{:02}:{:02}] {}:** {}\n\n",
            s / 60,
            s % 60,
            if u.source == "mic" { "Saya" } else { "Mereka" },
            u.text
        ));
    }
    out
}

fn inline_runs(text: &str) -> Vec<docx_rs::Run> {
    text.split("**")
        .enumerate()
        .filter(|(_, t)| !t.is_empty())
        .map(|(i, t)| {
            let r = docx_rs::Run::new().add_text(t);
            if i % 2 == 1 { r.bold() } else { r }
        })
        .collect()
}

fn markdown_to_docx(md: &str) -> docx_rs::Docx {
    use docx_rs::{Docx, Paragraph, Run};
    let mut doc = Docx::new();
    for line in md.lines() {
        let l = line.trim_end();
        let p = if let Some(h) = l.strip_prefix("# ") {
            Paragraph::new().add_run(Run::new().add_text(h).bold().size(36))
        } else if let Some(h) = l.strip_prefix("## ").or_else(|| l.strip_prefix("### ")) {
            Paragraph::new().add_run(Run::new().add_text(h).bold().size(28))
        } else if let Some(b) = l
            .trim_start()
            .strip_prefix("- ")
            .or_else(|| l.trim_start().strip_prefix("* "))
        {
            let mut p = Paragraph::new().add_run(Run::new().add_text("•  "));
            for r in inline_runs(b) {
                p = p.add_run(r);
            }
            p
        } else if l == "---" {
            Paragraph::new()
        } else {
            let mut p = Paragraph::new();
            for r in inline_runs(l) {
                p = p.add_run(r);
            }
            p
        };
        doc = doc.add_paragraph(p);
    }
    doc
}

/// Export the meeting (notulen + transcript) to `.docx` or `.md`.
#[tauri::command(async)]
pub fn export_meeting(
    state: State<'_, AppState>,
    meeting_id: i64,
    path: String,
) -> Result<String, String> {
    let d = get_meeting(state, meeting_id)?;
    let md = export_markdown(&d);
    if path.to_lowercase().ends_with(".docx") {
        let f = std::fs::File::create(&path).map_err(e)?;
        markdown_to_docx(&md).build().pack(f).map_err(e)?;
    } else {
        std::fs::write(&path, md).map_err(e)?;
    }
    Ok(path)
}

/// WhatsApp-formatted notulen (bold = *text*), ready for a `wa.me` share link.
fn whatsapp_text(d: &MeetingDetail) -> String {
    let notulen = d.meeting.notulen.clone().unwrap_or_default();
    let mut out = String::new();
    for line in notulen.lines() {
        let l = line.trim_end();
        let l = if let Some(h) = l
            .strip_prefix("# ")
            .or_else(|| l.strip_prefix("## "))
            .or_else(|| l.strip_prefix("### "))
        {
            format!("*{}*", h.replace("**", ""))
        } else {
            l.replace("**", "*")
        };
        out.push_str(&l);
        out.push('\n');
    }
    let mut out = out.trim().to_string();
    if out.chars().count() > 3500 {
        out = out.chars().take(3500).collect::<String>() + "\n…";
    }
    out
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tauri::command]
pub fn whatsapp_share_url(state: State<'_, AppState>, meeting_id: i64) -> Result<String, String> {
    let d = get_meeting(state, meeting_id)?;
    if d.meeting.notulen.is_none() {
        return Err("Notulen belum ada.".into());
    }
    Ok(format!(
        "https://wa.me/?text={}",
        percent_encode(&whatsapp_text(&d))
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(notulen: &str) -> MeetingDetail {
        MeetingDetail {
            meeting: Meeting {
                id: 1,
                title: "Rapat".into(),
                preset_id: "rapat".into(),
                app_name: None,
                started_at: 0,
                ended_at: None,
                notulen: Some(notulen.into()),
                recording_dir: None,
                utterances: 0,
                documents: 0,
            },
            date: String::new(),
            utterances: vec![StoredUtterance {
                source: "app".into(),
                text: "Halo semua.".into(),
                start: 65.0,
                end: 66.0,
            }],
            answers: vec![],
            documents: vec![],
        }
    }

    #[test]
    fn whatsapp_formatting() {
        let d = detail("# Rapat OKR\n## Keputusan\n- Target **20%** disetujui");
        assert_eq!(
            whatsapp_text(&d),
            "*Rapat OKR*\n*Keputusan*\n- Target *20%* disetujui"
        );
        assert_eq!(percent_encode("a b&*"), "a%20b%26%2A");
    }

    #[test]
    fn exports_markdown_and_docx() {
        let d = detail("# Rapat\n## Ringkasan\n- poin **penting**");
        let md = export_markdown(&d);
        assert!(md.contains("**[01:05] Mereka:** Halo semua."));
        let path = std::env::temp_dir().join(format!("bicara-export-{}.docx", std::process::id()));
        let f = std::fs::File::create(&path).unwrap();
        markdown_to_docx(&md).build().pack(f).unwrap();
        let text = bicara_docs::extract(&path).unwrap();
        assert!(
            text.contains("Ringkasan") && text.contains("• poin penting"),
            "{text}"
        );
        let _ = std::fs::remove_file(path);
    }
}
