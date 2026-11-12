# BICARA

Indonesian-first live AI meeting assistant (Tauri 2 + Rust + React). See [PLAN.md](PLAN.md).

## Phase 0 spike — run it

```bash
pnpm install
pnpm tauri dev                                  # dev mode
pnpm tauri build --debug --bundles app          # macOS .app (needed for permission prompts)
open target/debug/bundle/macos/BICARA.app
```

- **macOS 14.2+** for per-app capture. First capture asks for *System Audio Recording* permission.
- **Windows 10 2004+ / 11** for per-app capture (WASAPI process loopback).

## Layout

| Path | What |
|---|---|
| `crates/bicara-audio` | per-app capture (macOS Core Audio taps in `native/macos/tap.m`, Windows WASAPI loopback) + mic |
| `src-tauri/src/capture.rs` | capture session → WAV files + live `audio-level` events |
| `src-tauri/src/stealth.rs` | stealth mode + panic hotkey |
| `src/` | React UI (Indonesian) |
| `tools/stt-eval` | Indonesian STT benchmark (WER/CER) |

## Audio crate examples

```bash
cargo run -p bicara-audio --example list_apps
cargo run -p bicara-audio --example capture_app -- <app-id> 10 out.wav
```

## STT benchmark

```bash
cd tools/stt-eval
./make_sample_clips.sh                          # synthetic smoke-test clips (macOS "Damayanti" voice)
brew install whisper-cpp                        # local engine (optional)
WHISPER_MODEL=/path/ggml-large-v3-turbo.bin DEEPGRAM_API_KEY=... OPENAI_API_KEY=... \
  python3 eval.py clips --engines whisper_cpp deepgram openai
```

Put real meeting/interview recordings (recorded with BICARA) in `clips/` with a `.txt` reference transcript each.
