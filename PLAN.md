# BICARA — Indonesian-first live AI meeting assistant

Reference: NexQ (https://github.com/naxhq/NexQ) — Tauri 2 + Rust + React, captures mic + whole system audio.

## Differentiators vs NexQ
1. **Indonesian-first** — UI, live transcription (STT), question detection, answers. English later.
2. **Per-application audio capture** — user picks the app (Zoom, browser with Google Meet, Teams…), only that app's audio is captured.
3. **Subscription LLMs** — Claude (Pro/Max via user's `claude` CLI) and Codex (ChatGPT Free/Plus/Pro via user's `codex` CLI), plus API keys and local models.
4. **Stealth mode** — overlay hidden from screen share / recording.
5. Indonesia-specific features (see below).

Targets: **Windows 10 2004+ and macOS 14.4+** first. Linux later.

## Stack
- Tauri 2 (Rust backend, tokio) + React + TypeScript + Tailwind + shadcn/ui + Zustand
- SQLite (rusqlite) + FTS5 + sqlite-vec for history and RAG
- whisper.cpp (local STT), Silero VAD

## Audio capture
Two streams: **Mic ("Saya")** and **Target app ("Mereka")**.

| | Windows | macOS |
|---|---|---|
| API | WASAPI process loopback, `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` | Core Audio Process Taps (`AudioHardwareCreateProcessTap`); ScreenCaptureKit fallback for 13.x |
| Browser | Capture chrome.exe process tree (includes audio subprocess) | Group helper processes by bundle-ID prefix (e.g. `com.google.Chrome.helper`) |
| Permission | none | "System Audio Recording" |

- App picker lists only apps **currently playing audio** (Windows `IAudioSessionManager2`; macOS process object list + `kAudioProcessPropertyIsRunningOutput`) with icon + live level meter.
- Browser = whole browser. Per-tab capture later via companion extension (`chrome.tabCapture`).

## STT (Indonesian)
- Cloud real-time candidates: Soniox, Deepgram (`id`), Azure Speech `id-ID`, Google Chirp.
- Local: whisper.cpp `large-v3-turbo` + VAD chunking (Metal / CUDA / Vulkan).
- Must handle Indonesian–English code-switching.
- Custom vocabulary (names, products, jargon) → STT boosting + LLM post-correction.
- **Phase 0 eval:** 10–15 real Indonesian meeting/interview clips, WER per engine.
- Optional TTS later (Azure `id-ID` voices) to read suggestions into an earpiece.

## LLM providers
One `LlmProvider` trait: `stream(messages) -> tokens`.
1. Claude subscription — user's `claude` CLI, headless, `--input-format stream-json --output-format stream-json`, one persistent process per meeting. App never touches the OAuth token.
2. Codex subscription — user's `codex` CLI after `codex login` (`codex app-server` JSON-RPC or `codex exec --json`).
3. API keys — Anthropic, OpenAI, Gemini, Groq, OpenRouter (stored in OS keychain).
4. Local — Ollama, LM Studio.

Two tiers: **fast** model (question detection, short replies) + **smart** model (Assist / Summary / Ask).

⚠️ Before public release, re-check Anthropic and OpenAI terms on third-party use of subscriptions; keep API-key path first-class.

## Core features
- Assist, What to Say, Ask, Summary, Follow-up — streaming into always-on-top overlay.
- Indonesian question detection: rules (`?`, apa, bagaimana/gimana, kenapa, bisa jelaskan, menurut Anda, ceritakan…) → fast-model classifier → auto suggestion.
- RAG: PDF/DOCX/TXT/MD, SQLite FTS5 + multilingual embeddings (bge-m3 / multilingual-e5), context sets per meeting.
- Presets: custom system prompts, prompt templates, modes (Interview, Webinar, Rapat, Sales call).
- Meeting history + "Ask a past meeting".

## Indonesia-specific features (approved)
- **Formal vs casual toggle** (Bahasa baku "Bapak/Ibu" vs santai "kamu/gue").
- **Notulen export**: Keputusan, Action Items (PIC + deadline), Poin Diskusi → DOCX/PDF + **share to WhatsApp**.
- **HR interview pack**: common Indonesian HRD question bank + STAR answers from user's CV.
- **Live translation**: Indonesian ↔ English (Javanese/Sundanese later).
- **UU PDP**: consent reminder before recording; local-first storage by default.
- **Ask a past meeting**: search and Q&A over history.

## Stealth mode (approved)
Goal: BICARA overlay and windows invisible in screen share and recordings, while visible to the user.

| | Windows | macOS |
|---|---|---|
| Exclude from capture | `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` (Win10 2004+) — Tauri `setContentProtected(true)` | `NSWindow.sharingType = .none` — Tauri `setContentProtected(true)` |
| Hide from taskbar / Dock / Alt-Tab | `skipTaskbar`, `WS_EX_TOOLWINDOW` | `NSApplication.ActivationPolicy.accessory` (no Dock icon, not in Cmd-Tab) |

Extras:
- **Panic hotkey** — instantly hide/show all BICARA windows.
- **Click-through mode** — overlay ignores mouse so it never steals focus during a call.
- Adjustable opacity, compact mode, global shortcuts for Assist/Ask without focusing the app.
- **Self-test screen**: start a test capture and show the user whether BICARA is visible, per sharing app.

Caveat to verify in Phase 0: on recent macOS versions, ScreenCaptureKit-based capture (used by Zoom, Chrome/Meet, OBS) may not honour `sharingType = .none`. Test each target app + OS version; document what works.

## Roadmap
- **Phase 0 — spike (1–2 wk):** per-app audio capture on Win + Mac; app picker; STT WER test on Indonesian clips; stealth mode test matrix (Zoom, Meet in Chrome, Teams × Win/Mac).
- **Phase 1 — MVP:** dual-stream transcript; Claude CLI + Codex CLI + API-key providers; Assist/Ask/Summary overlay; presets & system prompts; stealth mode + panic hotkey.
- **Phase 2:** RAG; question auto-detection; notulen export + WhatsApp share; formal/casual toggle; HR interview pack; meeting history + ask past meeting; UU PDP consent.
- **Phase 3:** live translation; browser tab-capture extension; optional TTS; code signing (Apple notarization, Windows cert); auto-update.
- **Later:** Linux (PipeWire per-app capture); English UI/STT.

## Phase 0 status (2026-11-12)

| Item | Status | Evidence |
|---|---|---|
| macOS per-app capture (process tap) | ✅ verified | 48 kHz capture of target app; isolation test: after target stops, capture is silent (RMS 0.0000) while another app keeps playing |
| macOS app picker (only apps using audio, helpers grouped) | ✅ verified | Chrome helpers grouped under Google Chrome; daemons hidden unless playing |
| Windows process loopback + session listing | ⚠️ compiles only | `cargo check --target x86_64-pc-windows-msvc` passes; needs a run on real Windows 10 2004+/11 |
| Mic capture (cpal) | ⚠️ built, not exercised | needs mic permission click on first run |
| Live level meters + WAV recording | ✅ verified in bundled app | `~/Library/Application Support/id.bicara.desktop/recordings/session-*/` |
| Stealth (content protection + accessory policy) | ✅ macOS, own screenshot test | window & full-screen capture no longer show BICARA, Dock icon hidden |
| Stealth vs Zoom / Meet / Teams share | ⏳ manual | use the matrix in the app |
| Panic hotkey ⌘⇧H / Ctrl+Shift+H | ✅ macOS | hides and re-shows all windows |
| STT WER harness | ✅ built | `tools/stt-eval/eval.py`; whisper.cpp / Deepgram / OpenAI need install or keys; needs REAL Indonesian clips |

Known gaps / next:
- Capture targets the processes present at start; a helper that starts audio later (e.g. Chrome spawning a new audio process) is not added yet → re-tap on process-list change.
- Browser = whole browser (tab-level later via extension).
- Debug build is unsigned; System Audio Recording permission is attributed per build.
