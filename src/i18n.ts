import { createContext, useContext } from "react";

export type UiLang = "id" | "en";

/** Indonesian source text → English. Indonesian is the source language; missing keys fall back to it. */
const EN: Record<string, string> = {
  // navigation & common
  Meeting: "Meeting",
  Riwayat: "History",
  Pengaturan: "Settings",
  Siluman: "Stealth",
  Refresh: "Refresh",
  Tutup: "Close",
  Hapus: "Delete",
  Simpan: "Save",
  Batal: "Cancel",
  Edit: "Edit",
  Tanya: "Ask",
  Saya: "Me",
  Mereka: "Them",
  Notulen: "Minutes",
  Transkrip: "Transcript",
  "Jawaban AI": "AI answers",
  Pertanyaan: "Question",
  "Default sistem": "System default",
  "Bahasa Indonesia": "Bahasa Indonesia",
  English: "English",

  // meeting setup
  "Meeting baru": "New meeting",
  Judul: "Title",
  "Jenis (preset)": "Type (preset)",
  "mis. Interview PM — PT Maju": "e.g. PM interview — Acme Inc.",
  "Aplikasi meeting (audio “Mereka”)": "Meeting app (“Them” audio)",
  "— hanya mikrofon —": "— microphone only —",
  " · memutar audio": " · playing audio",
  "Mikrofon saya": "My microphone",
  "Mikrofon (“Saya”)": "Microphone (“Me”)",
  "Bahasa percakapan": "Conversation language",
  "Terjemahan langsung": "Live translation",
  "Mati": "Off",
  "Ke English": "To English",
  "Ke Bahasa Indonesia": "To Indonesian",
  "Tampilkan teks sementara saat orang masih bicara": "Show in-progress text while someone is still speaking",
  "Terjemahan memakai penyedia AI (Haiku untuk langganan Claude), satu baris setiap kali.": "Translation uses your AI provider (Haiku on a Claude subscription), one line at a time.",
  "Otomatis (ID / EN)": "Automatic (ID / EN)",
  "Untuk Google Meet pilih browser-nya. Aplikasi baru muncul setelah memakai audio.":
    "For Google Meet, pick your browser. An app appears here once it has used audio.",
  Dokumen: "Documents",
  "Dokumen konteks": "Context documents",
  "Tambah file": "Add files",
  "PDF, Word, PowerPoint, TXT, Markdown — mis. agenda, CV, deskripsi pekerjaan, materi presentasi. AI memakainya saat menjawab.":
    "PDF, Word, PowerPoint, TXT, Markdown — e.g. agenda, CV, job description, slides. The AI uses them when answering.",
  karakter: "characters",
  "rb karakter": "k characters",
  "Siapkan jawaban interview": "Prepare interview answers",
  "Jawaban STAR untuk 12 pertanyaan HRD umum, dari CV & deskripsi pekerjaan.":
    "STAR answers to 12 common HR questions, from the CV & job description.",
  "Peserta sudah saya beri tahu bahwa percakapan ini direkam & ditranskrip (UU PDP).":
    "I have told participants that this conversation is recorded & transcribed (UU PDP).",
  "Transkripsi siap": "Transcription ready",
  "Memuat model transkripsi…": "Loading transcription model…",
  "Mulai meeting": "Start meeting",
  "Membaca dokumen…": "Reading documents…",
  "Menyiapkan…": "Preparing…",

  // after meeting
  "Mengakhiri meeting…": "Ending meeting…",
  "Meeting selesai": "Meeting finished",
  "Buka di riwayat": "Open in history",
  "Menyusun notulen…": "Writing the minutes…",

  // live
  "Gaya bahasa": "Tone",
  Formal: "Formal",
  Santai: "Casual",
  "Bantu otomatis": "Auto-assist",
  Selesai: "End",
  "Transkrip muncul di sini begitu ada yang bicara.": "The transcript appears here as soon as someone speaks.",
  Asisten: "Assistant",
  "Asisten AI": "AI assistant",
  "Pertanyaan terdeteksi": "Detected question",
  "Tekan {action} atau {key} kapan saja.": "Press {action} or {key} at any time.",

  // AI panel & overlay
  "Bantu jawab": "Help me answer",
  "Pertanyaan lanjutan": "Follow-up questions",
  Ringkas: "Summarize",
  "Tanya AI soal percakapan ini…": "Ask the AI about this conversation…",
  "Pertanyaan untuk AI": "Question for the AI",
  "Sedang berpikir…": "Thinking…",
  dtk: "s",
  "Pintasan {key}": "Shortcut {key}",
  "bantu jawab": "help me answer",
  sembunyikan: "hide",
  Ringkasan: "Summary",
  "Persiapan interview": "Interview prep",
  "Tanya meeting": "Ask meeting",

  // history
  "Cari di semua meeting…": "Search all meetings…",
  "Cari meeting": "Search meetings",
  "Belum ada meeting.": "No meetings yet.",
  "Tidak ditemukan.": "Nothing found.",
  kalimat: "lines",
  " · belum selesai": " · not finished",
  "Pilih meeting di kiri.": "Pick a meeting on the left.",
  menit: "min",
  "Dokumen:": "Documents:",
  "Belum ada notulen.": "No minutes yet.",
  "Buat ulang notulen": "Regenerate minutes",
  "Buka folder rekaman": "Open recordings folder",
  "Tanya tentang meeting ini, mis. “Apa tenggat yang disepakati?”":
    "Ask about this meeting, e.g. “Which deadline did we agree on?”",
  "Tanya tentang meeting ini": "Ask about this meeting",
  "Tersimpan: {path}": "Saved: {path}",
  'Hapus "{title}" beserta transkrip, notulen, dokumen, dan rekamannya? Tidak bisa dibatalkan.':
    'Delete "{title}" with its transcript, minutes, documents and recordings? This cannot be undone.',

  // settings
  "Model AI": "AI model",
  "Penyedia AI": "AI provider",
  "Claude (langganan)": "Claude (subscription)",
  "Codex / ChatGPT (langganan)": "Codex / ChatGPT (subscription)",
  "Anthropic API": "Anthropic API",
  "OpenAI-compatible": "OpenAI-compatible",
  "CLI ditemukan": "CLI found",
  "CLI `claude` belum terpasang": "`claude` CLI not installed",
  "CLI `codex` belum terpasang": "`codex` CLI not installed",
  "pakai API key": "uses an API key",
  "OpenAI, Groq, OpenRouter, Ollama, LM Studio": "OpenAI, Groq, OpenRouter, Ollama, LM Studio",
  Model: "Model",
  "Haiku — paling cepat": "Haiku — fastest",
  "Sonnet — seimbang": "Sonnet — balanced",
  "Opus — paling pintar, lebih lambat": "Opus — smartest, slower",
  "Memakai login Claude Code kamu sendiri (Pro/Max). BICARA tidak membaca token kamu. Belum login? Jalankan":
    "Uses your own Claude Code login (Pro/Max). BICARA never reads your token. Not logged in? Run",
  "sekali di terminal.": "once in a terminal.",
  "Ditemukan:": "Found:",
  "Model (kosongkan untuk default)": "Model (leave empty for default)",
  "Memakai login ChatGPT di Codex CLI (gratis atau berbayar). Pasang Codex CLI lalu jalankan":
    "Uses your ChatGPT login in the Codex CLI (free or paid). Install the Codex CLI, then run",
  "Codex mengirim jawaban sekaligus (tidak per kata).": "Codex sends the whole answer at once (not word by word).",
  Effort: "Effort",
  "Low — tercepat": "Low — fastest",
  Medium: "Medium",
  High: "High",
  Layanan: "Service",
  "Base URL": "Base URL",
  "API key": "API key",
  "(tersimpan di keychain — isi untuk mengganti)": "(saved in the keychain — type to replace)",
  "tempel API key": "paste API key",
  "Bahasa & mikrofon": "Language & microphone",
  "Bahasa aplikasi": "App language",
  "Bahasa percakapan (transkripsi & jawaban AI)": "Conversation language (transcription & AI answers)",
  "Otomatis (Indonesia / English)": "Automatic (Indonesian / English)",
  Mikrofon: "Microphone",
  "“Otomatis” cocok untuk meeting campuran, tapi sedikit kurang akurat dibanding memilih satu bahasa.":
    "“Automatic” suits mixed-language meetings but is slightly less accurate than picking one language.",
  "Transkripsi (offline, whisper)": "Transcription (offline, whisper)",
  Terpasang: "Installed",
  "Large v3 Turbo (q5) — direkomendasikan": "Large v3 Turbo (q5) — recommended",
  "Large v3 Turbo (penuh) — paling akurat": "Large v3 Turbo (full) — most accurate",
  "Small — untuk laptop lambat": "Small — for slower laptops",
  Unduh: "Download",
  "Unduhan gagal: {error}": "Download failed: {error}",
  "Kosakata khusus (nama orang, produk, istilah) — membantu akurasi":
    "Custom vocabulary (names, products, jargon) — improves accuracy",
  "contoh: Tokopedia, OKR, Pak Budi, sprint review": "e.g. Tokopedia, OKR, Mr. Budi, sprint review",
  "Preset & system prompt": "Presets & system prompt",
  "Tambah preset": "Add preset",
  Preset: "Preset",
  "(aktif)": "(active)",
  Nama: "Name",
  "System prompt (peran & aturan AI)": "System prompt (AI role & rules)",
  "Konteks (tempel CV, deskripsi pekerjaan, agenda, catatan produk…)":
    "Context (paste CV, job description, agenda, product notes…)",
  "Teks ini dikirim ke AI bersama transkrip.": "This text is sent to the AI together with the transcript.",
  "Jadikan aktif": "Make active",
  "Kembalikan prompt bawaan": "Restore built-in prompt",
  "Hapus preset": "Delete preset",
  "Preset baru": "New preset",
  "Kamu adalah asisten percakapan yang membantu pengguna.": "You are a conversation assistant that helps the user.",
  Privasi: "Privacy",
  "Ingatkan saya memberi tahu peserta sebelum merekam (UU PDP)":
    "Remind me to tell participants before recording (UU PDP)",
  "Simpan rekaman audio setiap meeting (transkrip & notulen selalu disimpan)":
    "Keep the audio recording of each meeting (transcripts & minutes are always kept)",
  "Semua data tersimpan lokal di komputer ini. Teks transkrip dikirim ke penyedia AI yang kamu pilih saat AI dipakai.":
    "All data stays on this computer. Transcript text is sent to the AI provider you chose when the AI is used.",
  "Simpan pengaturan": "Save settings",
  "Tersimpan.": "Saved.",
  "Gagal menyimpan: {error}": "Could not save: {error}",

  // messages from the backend
  "(langganan)": "(subscription)",
  "Masih ada meeting yang berjalan. Selesaikan dulu.": "A meeting is still running. End it first.",
  "Meeting ini sedang berjalan.": "This meeting is still running.",
  "Notulen belum ada.": "There are no minutes yet.",
  "Pilih aplikasi atau aktifkan mikrofon.": "Pick an app or turn on the microphone.",
  "Rekaman sedang berjalan.": "A recording is already running.",
  "API key Anthropic belum diisi": "The Anthropic API key is not set",
  "CLI `claude` tidak ditemukan. Install Claude Code lalu jalankan `claude` sekali untuk login.":
    "`claude` CLI not found. Install Claude Code, then run `claude` once to log in.",
  "CLI `codex` tidak ditemukan. Install Codex CLI lalu jalankan `codex login`.":
    "`codex` CLI not found. Install the Codex CLI, then run `codex login`.",
  "Model menolak permintaan ini (refusal).": "The model declined this request (refusal).",
  "Belum ada percakapan. Mulai meeting dan tunggu lawan bicara berbicara.":
    "No conversation yet. Start the meeting and wait for the other side to speak.",
  "Unggah CV dan/atau deskripsi pekerjaan dulu supaya jawabannya personal.":
    "Upload a CV and/or job description first so the answers are personal.",
  "Tidak ada transkrip, jadi notulen tidak dibuat.": "There is no transcript, so no minutes were written.",
  "Aplikasi ini belum memakai audio. Mulai meeting/putar suara dulu, lalu refresh daftar.":
    "This app hasn't used audio yet. Start the meeting / play sound first, then refresh the list.",
  "Capture per aplikasi butuh macOS 14.2 atau lebih baru.": "Per-app capture needs macOS 14.2 or later.",
  "Tidak menemukan output device default.": "No default output device found.",
  "mic: tidak ada mikrofon": "mic: no microphone found",

  // built-in presets (shown by id)
  "Rapat kerja": "Work meeting",
  "Interview kerja (saya kandidat)": "Job interview (I'm the candidate)",
  "Interview kerja (saya pewawancara)": "Job interview (I'm the interviewer)",
  "Diskusi klien / konsultasi": "Client discussion / consulting",
  "Webinar / presentasi": "Webinar / presentation",
  "Sales call": "Sales call",

  // stealth
  "Mode siluman": "Stealth mode",
  "Aktifkan stealth": "Turn on stealth",
  "Stealth AKTIF": "Stealth ON",
  "Saat aktif, jendela BICARA disembunyikan dari share layar & rekaman, dan tidak muncul di taskbar / Dock.":
    "When on, BICARA's windows are hidden from screen sharing & recordings, and don't appear in the taskbar / Dock.",
  "menyembunyikan / menampilkan semua jendela.": "hides / shows all windows.",
  "meminta jawaban AI tanpa membuka jendela.": "asks the AI for an answer without opening a window.",
  "Uji: aktifkan stealth → share layar di aplikasi meeting → cek preview → catat hasilnya di bawah.":
    "Test: turn on stealth → share your screen in the meeting app → check the preview → record the result below.",
  Aplikasi: "App",
  "Share layar penuh": "Share full screen",
  "Share jendela": "Share window",
  "Belum diuji": "Not tested",
  "Tersembunyi ✓": "Hidden ✓",
  "Terlihat ✗": "Visible ✗",
  "OBS / rekam layar": "OBS / screen recording",
  "Kalau jendela ini terlihat di layar yang kamu share / rekam, berarti":
    "If this window is visible in what you share / record,",
  "stealth gagal": "stealth failed",
  "untuk aplikasi itu.": "for that app.",
};

const DICTS: Record<UiLang, Record<string, string>> = { id: {}, en: EN };

export function translate(lang: UiLang, key: string, vars?: Record<string, string | number>): string {
  let s = DICTS[lang][key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.split(`{${k}}`).join(String(v));
  return s;
}

export const LangContext = createContext<UiLang>("id");

export type T = (key: string, vars?: Record<string, string | number>) => string;

export function useT(): T {
  const lang = useContext(LangContext);
  return (key, vars) => translate(lang, key, vars);
}

/** Built-in preset names follow the UI language; custom presets keep the user's name. */
const BUILTIN_PRESET_NAMES: Record<string, string> = {
  rapat: "Rapat kerja",
  interview: "Interview kerja (saya kandidat)",
  interviewer: "Interview kerja (saya pewawancara)",
  klien: "Diskusi klien / konsultasi",
  webinar: "Webinar / presentasi",
  sales: "Sales call",
};

export function presetName(t: T, p: { id: string; name: string; builtin?: boolean }) {
  const base = BUILTIN_PRESET_NAMES[p.id];
  return p.builtin && base && p.name === base ? t(base) : p.name;
}
