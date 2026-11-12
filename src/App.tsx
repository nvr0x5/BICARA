import { useCallback, useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, AudioApp, useLevels } from "./api";
import { Meter } from "./Meter";

const MATRIX_APPS = ["Zoom", "Google Meet (Chrome)", "Microsoft Teams", "OBS / rekam layar"];
const MATRIX_MODES = ["Share layar penuh", "Share jendela"];
type Verdict = "untested" | "hidden" | "visible";
const VERDICT_LABEL: Record<Verdict, string> = {
  untested: "Belum diuji",
  hidden: "Tersembunyi ✓",
  visible: "Terlihat ✗",
};
const NEXT: Record<Verdict, Verdict> = { untested: "hidden", hidden: "visible", visible: "untested" };

function loadMatrix(): Record<string, Verdict> {
  try {
    return JSON.parse(localStorage.getItem("bicara.stealthMatrix") || "{}");
  } catch {
    return {};
  }
}

export default function App() {
  const [apps, setApps] = useState<AudioApp[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [withMic, setWithMic] = useState(true);
  const [recording, setRecording] = useState(false);
  const [files, setFiles] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [stealth, setStealth] = useState(false);
  const [hotkey, setHotkey] = useState("");
  const [matrix, setMatrix] = useState<Record<string, Verdict>>(loadMatrix);
  const levels = useLevels();

  const refresh = useCallback(async () => {
    try {
      setApps(await api.listAudioApps());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    api.getStealth().then(setStealth);
    api.panicHotkeyLabel().then(setHotkey);
  }, [refresh]);

  // Keep the "playing" badges fresh while idle.
  useEffect(() => {
    if (recording) return;
    const t = setInterval(refresh, 3000);
    return () => clearInterval(t);
  }, [recording, refresh]);

  const start = async () => {
    setError(null);
    setFiles([]);
    try {
      await api.startCapture(selected, withMic);
      setRecording(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const stop = async () => {
    try {
      setFiles(await api.stopCapture());
    } catch (e) {
      setError(String(e));
    }
    setRecording(false);
  };

  const toggleStealth = async () => {
    try {
      setStealth(await api.setStealth(!stealth));
    } catch (e) {
      setError(String(e));
    }
  };

  const cycle = (key: string) => {
    const next = { ...matrix, [key]: NEXT[matrix[key] ?? "untested"] };
    setMatrix(next);
    try {
      localStorage.setItem("bicara.stealthMatrix", JSON.stringify(next));
    } catch {
      /* storage unavailable */
    }
  };

  const selectedApp = apps.find((a) => a.id === selected);

  return (
    <main className="app">
      <header className="top">
        <div>
          <h1>BICARA</h1>
          <p className="sub">Phase 0 · uji capture audio per aplikasi & mode siluman</p>
        </div>
        <span className={`pill ${recording ? "rec" : ""}`}>{recording ? "● Merekam" : "Siap"}</span>
      </header>

      {error && (
        <div className="error" role="alert">
          {error}
          <button className="link" onClick={() => setError(null)}>
            Tutup
          </button>
        </div>
      )}

      <section className="card">
        <div className="card-head">
          <h2>1 · Pilih aplikasi meeting</h2>
          <button className="ghost" onClick={refresh} disabled={recording}>
            Refresh
          </button>
        </div>
        <p className="hint">
          Hanya aplikasi yang sudah memakai audio yang muncul. Buka Zoom / Google Meet dulu, lalu refresh.
          Untuk Google Meet, pilih browser-nya.
        </p>
        <ul className="apps">
          {apps.length === 0 && <li className="empty">Belum ada aplikasi yang memakai audio.</li>}
          {apps.map((a) => (
            <li key={a.id}>
              <button
                className={`app-row ${selected === a.id ? "sel" : ""}`}
                onClick={() => setSelected(selected === a.id ? null : a.id)}
                disabled={recording}
              >
                {a.icon_png_base64 ? (
                  <img src={`data:image/png;base64,${a.icon_png_base64}`} alt="" />
                ) : (
                  <span className="noicon">{a.name.slice(0, 1).toUpperCase()}</span>
                )}
                <span className="app-name">{a.name}</span>
                {a.playing && <span className="badge">memutar audio</span>}
                <span className="app-id">{a.id}</span>
              </button>
            </li>
          ))}
        </ul>
        <label className="check">
          <input type="checkbox" checked={withMic} onChange={(e) => setWithMic(e.target.checked)} disabled={recording} />
          Rekam mikrofon juga (“Saya”)
        </label>
      </section>

      <section className="card">
        <div className="card-head">
          <h2>2 · Rekam</h2>
          {recording ? (
            <button className="danger" onClick={stop}>
              Berhenti
            </button>
          ) : (
            <button className="primary" onClick={start} disabled={!selected && !withMic}>
              Mulai{selectedApp ? ` · ${selectedApp.name}` : ""}
            </button>
          )}
        </div>
        <Meter label="Mereka" rms={levels.app?.rms} peak={levels.app?.peak} />
        <Meter label="Saya" rms={levels.mic?.rms} peak={levels.mic?.peak} />
        {files.length > 0 && (
          <div className="files">
            <p>Tersimpan (WAV mono, dipakai untuk uji STT):</p>
            <ul>
              {files.map((f) => (
                <li key={f}>
                  <code>{f}</code>
                </li>
              ))}
            </ul>
            <button className="ghost" onClick={() => revealItemInDir(files[0])}>
              Buka folder
            </button>
          </div>
        )}
      </section>

      <section className="card">
        <div className="card-head">
          <h2>3 · Mode siluman (stealth)</h2>
          <button className={stealth ? "primary" : "ghost"} onClick={toggleStealth} aria-pressed={stealth}>
            {stealth ? "Stealth AKTIF" : "Aktifkan stealth"}
          </button>
        </div>
        <p className="hint">
          Saat aktif, jendela BICARA disembunyikan dari share layar & rekaman, dan tidak muncul di
          taskbar / Dock. Tombol panik <kbd>{hotkey}</kbd> menyembunyikan / menampilkan semua jendela.
        </p>
        <p className="hint">
          Cara uji: aktifkan stealth → mulai share layar di aplikasi meeting → lihat preview / minta
          lawan bicara cek → klik sel di bawah untuk mencatat hasilnya.
        </p>
        <table className="matrix">
          <thead>
            <tr>
              <th>Aplikasi</th>
              {MATRIX_MODES.map((m) => (
                <th key={m}>{m}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {MATRIX_APPS.map((a) => (
              <tr key={a}>
                <td>{a}</td>
                {MATRIX_MODES.map((m) => {
                  const k = `${a}|${m}`;
                  const v = matrix[k] ?? "untested";
                  return (
                    <td key={m}>
                      <button className={`verdict ${v}`} onClick={() => cycle(k)}>
                        {VERDICT_LABEL[v]}
                      </button>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </main>
  );
}
