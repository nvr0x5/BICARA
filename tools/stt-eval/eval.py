#!/usr/bin/env python3
"""BICARA Phase 0 — Indonesian STT benchmark.

Put clips in a folder:   clips/rapat-01.wav + clips/rapat-01.txt (reference transcript)
Run:                     python3 eval.py clips --engines whisper_cpp deepgram openai

Engines (each skipped with a note if its requirement is missing):
  whisper_cpp  local, needs `whisper-cli` on PATH and WHISPER_MODEL=/path/ggml-large-v3-turbo.bin
  deepgram     cloud, needs DEEPGRAM_API_KEY      (DEEPGRAM_MODEL, default nova-3)
  openai       cloud, needs OPENAI_API_KEY        (OPENAI_STT_MODEL, default gpt-4o-transcribe)

Output: per-clip and average WER / CER, plus results.csv. No third-party Python packages needed;
ffmpeg is used to convert audio to 16 kHz mono PCM.
"""
import argparse, csv, json, os, re, shutil, subprocess, sys, tempfile, time, unicodedata, uuid
import urllib.request

# ---------------------------------------------------------------- normalisation & metrics

NUM_WORDS = {  # keep digits and words comparable for common cases
    "0": "nol", "1": "satu", "2": "dua", "3": "tiga", "4": "empat",
    "5": "lima", "6": "enam", "7": "tujuh", "8": "delapan", "9": "sembilan", "10": "sepuluh",
}

def normalize(text: str) -> list[str]:
    t = unicodedata.normalize("NFKC", text).lower()
    t = re.sub(r"[’']", "", t)
    t = re.sub(r"[^\w\s-]", " ", t)       # drop punctuation, keep hyphenated reduplication (kata-kata)
    t = t.replace("_", " ")
    return [NUM_WORDS.get(w, w) for w in t.split()]

def edit_distance(a, b) -> int:
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i] + [0] * len(b)
        for j, y in enumerate(b, 1):
            cur[j] = min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y))
        prev = cur
    return prev[-1]

def wer(ref: str, hyp: str) -> float:
    r, h = normalize(ref), normalize(hyp)
    return edit_distance(r, h) / max(1, len(r))

def cer(ref: str, hyp: str) -> float:
    r, h = " ".join(normalize(ref)), " ".join(normalize(hyp))
    return edit_distance(list(r), list(h)) / max(1, len(r))

# ---------------------------------------------------------------- audio

def to_wav16k(src: str, tmpdir: str) -> str:
    out = os.path.join(tmpdir, os.path.splitext(os.path.basename(src))[0] + ".16k.wav")
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", src, "-ac", "1", "-ar", "16000",
                    "-c:a", "pcm_s16le", out], check=True)
    return out

# ---------------------------------------------------------------- engines

def eng_whisper_cpp(wav16):
    exe = shutil.which("whisper-cli")
    model = os.environ.get("WHISPER_MODEL")
    if not exe or not model:
        raise RuntimeError("butuh `whisper-cli` di PATH dan env WHISPER_MODEL")
    p = subprocess.run([exe, "-m", model, "-l", "id", "-nt", "-np", "-f", wav16],
                       capture_output=True, text=True, check=True)
    return p.stdout.strip()

def eng_deepgram(wav16):
    key = os.environ.get("DEEPGRAM_API_KEY")
    if not key:
        raise RuntimeError("butuh env DEEPGRAM_API_KEY")
    model = os.environ.get("DEEPGRAM_MODEL", "nova-3")
    req = urllib.request.Request(
        f"https://api.deepgram.com/v1/listen?model={model}&language=id&smart_format=true",
        data=open(wav16, "rb").read(),
        headers={"Authorization": f"Token {key}", "Content-Type": "audio/wav"})
    with urllib.request.urlopen(req, timeout=120) as r:
        j = json.load(r)
    return j["results"]["channels"][0]["alternatives"][0]["transcript"]

def eng_openai(wav16):
    key = os.environ.get("OPENAI_API_KEY")
    if not key:
        raise RuntimeError("butuh env OPENAI_API_KEY")
    model = os.environ.get("OPENAI_STT_MODEL", "gpt-4o-transcribe")
    b = uuid.uuid4().hex
    parts = []
    for k, v in (("model", model), ("language", "id"), ("response_format", "json")):
        parts.append(f'--{b}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode())
    parts.append(f'--{b}\r\nContent-Disposition: form-data; name="file"; filename="a.wav"\r\n'
                 f"Content-Type: audio/wav\r\n\r\n".encode() + open(wav16, "rb").read() + b"\r\n")
    parts.append(f"--{b}--\r\n".encode())
    req = urllib.request.Request("https://api.openai.com/v1/audio/transcriptions", data=b"".join(parts),
                                 headers={"Authorization": f"Bearer {key}",
                                          "Content-Type": f"multipart/form-data; boundary={b}"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.load(r)["text"]

def eng_reference(wav16, ref=""):
    """Sanity engine: returns the reference itself (WER must be 0)."""
    return ref

ENGINES = {"whisper_cpp": eng_whisper_cpp, "deepgram": eng_deepgram, "openai": eng_openai,
           "reference": eng_reference}

# ---------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("clips")
    ap.add_argument("--engines", nargs="+", default=["whisper_cpp", "deepgram", "openai"])
    ap.add_argument("--csv", default="results.csv")
    a = ap.parse_args()

    clips = sorted(f for f in os.listdir(a.clips)
                   if f.lower().endswith((".wav", ".m4a", ".mp3", ".aiff", ".flac"))
                   and os.path.exists(os.path.join(a.clips, os.path.splitext(f)[0] + ".txt")))
    if not clips:
        sys.exit(f"Tidak ada pasangan audio + .txt di {a.clips}")

    rows, totals = [], {}
    with tempfile.TemporaryDirectory() as tmp:
        for clip in clips:
            ref = open(os.path.join(a.clips, os.path.splitext(clip)[0] + ".txt"), encoding="utf-8").read()
            wav16 = to_wav16k(os.path.join(a.clips, clip), tmp)
            for name in a.engines:
                fn = ENGINES[name]
                t0 = time.time()
                try:
                    hyp = fn(wav16, ref) if name == "reference" else fn(wav16)
                except Exception as e:
                    print(f"  [{name}] {clip}: dilewati — {e}")
                    continue
                dt = time.time() - t0
                w, c = wer(ref, hyp), cer(ref, hyp)
                rows.append({"clip": clip, "engine": name, "wer": round(w, 4), "cer": round(c, 4),
                             "seconds": round(dt, 2), "hypothesis": hyp})
                totals.setdefault(name, []).append((w, c))
                print(f"  [{name:<11}] {clip:<28} WER {w:6.1%}  CER {c:6.1%}  ({dt:.1f}s)")

    if rows:
        with open(a.csv, "w", newline="", encoding="utf-8") as f:
            wr = csv.DictWriter(f, fieldnames=list(rows[0]))
            wr.writeheader(); wr.writerows(rows)
        print("\nRata-rata (lebih rendah = lebih baik):")
        for name, vals in sorted(totals.items(), key=lambda kv: sum(v[0] for v in kv[1]) / len(kv[1])):
            print(f"  {name:<11} WER {sum(v[0] for v in vals)/len(vals):6.1%}  "
                  f"CER {sum(v[1] for v in vals)/len(vals):6.1%}  n={len(vals)}")
        print(f"\nDetail: {a.csv}")

if __name__ == "__main__":
    main()
