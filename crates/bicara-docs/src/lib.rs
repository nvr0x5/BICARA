//! Meeting context documents: extract plain text and split it into retrieval chunks.
//! Supported: PDF, DOCX, PPTX, TXT, MD (and other plain-text files).

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("format file belum didukung: {0}")]
    Unsupported(String),
    #[error("gagal membaca {0}: {1}")]
    Read(String, String),
    #[error("tidak ada teks yang bisa dibaca di {0} (PDF hasil scan?)")]
    Empty(String),
}

pub fn supported_extensions() -> &'static [&'static str] {
    &["pdf", "docx", "pptx", "txt", "md", "markdown", "csv"]
}

/// Extract readable text from a file.
pub fn extract(path: &Path) -> Result<String, DocError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let rd = |e: &dyn std::fmt::Display| DocError::Read(name.clone(), e.to_string());
    let text = match ext.as_str() {
        "pdf" => {
            let bytes = std::fs::read(path).map_err(|e| rd(&e))?;
            pdf_extract::extract_text_from_mem(&bytes).map_err(|e| rd(&e))?
        }
        "docx" => office_text(path, |n| n == "word/document.xml", b"p").map_err(|e| rd(&e))?,
        "pptx" => office_text(
            path,
            |n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"),
            b"p",
        )
        .map_err(|e| rd(&e))?,
        "txt" | "md" | "markdown" | "csv" | "" => {
            let bytes = std::fs::read(path).map_err(|e| rd(&e))?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        other => return Err(DocError::Unsupported(format!(".{other}"))),
    };
    let text = normalize(&text);
    if text.trim().is_empty() {
        return Err(DocError::Empty(name));
    }
    Ok(text)
}

/// Read `<a:t>`/`<w:t>` text runs from the matching XML parts of an Office zip file,
/// starting a new line at every paragraph end (`</w:p>` / `</a:p>`).
fn office_text(path: &Path, want: impl Fn(&str) -> bool, para: &[u8]) -> Result<String, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|z| z.name().to_string()))
        .filter(|n| want(n))
        .collect();
    // slide2.xml before slide10.xml
    names.sort_by_key(|n| {
        let digits: String = n.chars().filter(|c| c.is_ascii_digit()).collect();
        (digits.parse::<u32>().unwrap_or(0), n.clone())
    });
    let mut out = String::new();
    for n in names {
        let mut xml = String::new();
        zip.by_name(&n)
            .map_err(|e| e.to_string())?
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
        let mut r = Reader::from_str(&xml);
        let mut in_text = false;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) if e.local_name().as_ref() == b"t" => in_text = true,
                Ok(Event::End(e)) if e.local_name().as_ref() == b"t" => in_text = false,
                Ok(Event::End(e)) if e.local_name().as_ref() == para => out.push('\n'),
                Ok(Event::Empty(e)) if e.local_name().as_ref() == b"tab" => out.push('\t'),
                Ok(Event::Empty(e)) if e.local_name().as_ref() == b"br" => out.push('\n'),
                Ok(Event::Text(t)) if in_text => {
                    out.push_str(&t.unescape().map_err(|e| e.to_string())?)
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(e.to_string()),
                _ => {}
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn normalize(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut blank = 0;
    for line in t.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.is_empty() {
            blank += 1;
            if blank == 1 {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(&l);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Split text into ~`size`-character chunks on paragraph/sentence boundaries,
/// with `overlap` characters carried over so an answer isn't cut in half.
pub fn chunk(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let mut pieces: Vec<&str> = Vec::new();
    for para in text.split("\n\n") {
        if para.chars().count() <= size {
            pieces.push(para);
        } else {
            // Split long paragraphs on sentence ends.
            let mut start = 0;
            for (i, c) in para.char_indices() {
                if matches!(c, '.' | '?' | '!' | '\n') {
                    let end = i + c.len_utf8();
                    pieces.push(&para[start..end]);
                    start = end;
                }
            }
            if start < para.len() {
                pieces.push(&para[start..]);
            }
        }
    }
    let mut chunks = Vec::new();
    let mut cur = String::new();
    for p in pieces.into_iter().map(str::trim).filter(|p| !p.is_empty()) {
        if !cur.is_empty() && cur.chars().count() + p.chars().count() + 1 > size {
            chunks.push(cur.clone());
            let tail: String = {
                let chars: Vec<char> = cur.chars().collect();
                chars[chars.len().saturating_sub(overlap)..]
                    .iter()
                    .collect()
            };
            // Start the overlap at a word boundary.
            cur = tail
                .split_once(' ')
                .map(|(_, r)| r.to_string())
                .unwrap_or_default();
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        // A single piece longer than `size` (no punctuation) is hard-split.
        let mut rest = p;
        while rest.chars().count() > size {
            let cut = rest
                .char_indices()
                .nth(size)
                .map(|(i, _)| i)
                .unwrap_or(rest.len());
            cur.push_str(&rest[..cut]);
            chunks.push(std::mem::take(&mut cur));
            rest = &rest[cut..];
        }
        cur.push_str(rest);
    }
    if !cur.trim().is_empty() {
        chunks.push(cur);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_respect_size_and_overlap() {
        let text = "Kalimat pertama tentang anggaran. ".repeat(60);
        let c = chunk(&text, 300, 60);
        assert!(c.len() > 5);
        assert!(
            c.iter().all(|x| x.chars().count() <= 360),
            "{:?}",
            c.iter().map(|x| x.len()).collect::<Vec<_>>()
        );
        // Overlap: the start of chunk 2 appears at the end of chunk 1.
        let head: String = c[1].chars().take(20).collect();
        assert!(c[0].contains(head.trim()));
    }

    #[test]
    fn short_text_one_chunk() {
        assert_eq!(
            chunk("Agenda: review OKR Q4.", 800, 150),
            vec!["Agenda: review OKR Q4."]
        );
    }

    #[test]
    fn extracts_docx_and_txt() {
        let dir = std::env::temp_dir().join(format!("bicara-docs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let docx = dir.join("cv.docx");
        let f = std::fs::File::create(&docx).unwrap();
        docx_rs::Docx::new()
            .add_paragraph(
                docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Budi Santoso")),
            )
            .add_paragraph(
                docx_rs::Paragraph::new().add_run(
                    docx_rs::Run::new().add_text("Product Manager di Tokopedia, 2021–2024"),
                ),
            )
            .build()
            .pack(f)
            .unwrap();
        let t = extract(&docx).unwrap();
        assert_eq!(t, "Budi Santoso\nProduct Manager di Tokopedia, 2021–2024");

        let txt = dir.join("agenda.md");
        std::fs::write(&txt, "# Agenda\n\n\n\n- Review   OKR\n").unwrap();
        assert_eq!(extract(&txt).unwrap(), "# Agenda\n\n- Review OKR");

        assert!(matches!(
            extract(&dir.join("x.exe")),
            Err(DocError::Unsupported(_))
        ));
        let _ = std::fs::remove_dir_all(dir);
    }
}
