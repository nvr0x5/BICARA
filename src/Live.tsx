import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, AudioApp, fmtTime, Settings, useAnswers, useLastQuestion, useLevels, useTranscript } from "./api";
import { Meter } from "./Meter";
import { ActionBar, AnswerCard } from "./AiPanel";

export default function Live({
  settings,
  onSettings,
  assistKey,
}: {
  settings: Settings | null;
  onSettings: (s: Settings) => void;
  assistKey: string;
}) {
  const [apps, setApps] = useState<AudioApp[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [withMic, setWithMic] = useState(true);
  const [recording, setRecording] = useState(false);
  const [starting, setStarting] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [sttReady, setSttReady] = useState(false);
  const levels = useLevels();
  const [lines, setLines] = useTranscript();
  const answers = useAnswers();
  const question = useLastQuestion();
  const endRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    try {
      setApps(await api.listAudioApps());
    } catch (e) {
      setNotice(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    api.sttLoaded().then(setSttReady);
    const un = listen("stt-ready", () => setSttReady(true));
    return () => {
      un.then((f) => f());
    };
  }, [refresh]);

  useEffect(() => {
    if (recording) return;
    const t = setInterval(refresh, 4000);
    return () => clearInterval(t);
  }, [recording, refresh]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [lines.length]);

  const start = async () => {
    setNotice(null);
    setStarting(true);
    try {
      const r = await api.startCapture(selected || null, withMic);
      setRecording(true);
      setSttReady(r.transcribing || sttReady);
      if (r.warning) setNotice(r.warning);
    } catch (e) {
      setNotice(String(e));
    }
    setStarting(false);
  };

  const stop = async () => {
    try {
      await api.stopCapture();
    } catch (e) {
      setNotice(String(e));
    }
    setRecording(false);
  };

  const update = (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    onSettings(next);
    api.saveSettings(next).catch((e) => setNotice(String(e)));
  };

  const sel = apps.find((a) => a.id === selected);

  return (
    <div className="live">
      <section className="card source">
        <div className="source-row">
          <label className="field">
            <span>Aplikasi meeting</span>
            <select value={selected} onChange={(e) => setSelected(e.target.value)} disabled={recording}>
              <option value="">— hanya mikrofon —</option>
              {apps.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                  {a.playing ? " · memutar audio" : ""}
                </option>
              ))}
            </select>
          </label>
          <button className="ghost" onClick={refresh} disabled={recording} aria-label="Muat ulang daftar aplikasi">
            Refresh
          </button>
          <label className="check">
            <input type="checkbox" checked={withMic} onChange={(e) => setWithMic(e.target.checked)} disabled={recording} />
            Mikrofon saya
          </label>
          {recording ? (
            <button className="danger" onClick={stop}>
              Berhenti
            </button>
          ) : (
            <button className="primary" onClick={start} disabled={starting || (!selected && !withMic)}>
              {starting ? "Menyiapkan…" : "Mulai"}
            </button>
          )}
        </div>
        <div className="meters">
          <Meter label={sel ? sel.name : "Mereka"} rms={levels.app?.rms} peak={levels.app?.peak} />
          <Meter label="Saya" rms={levels.mic?.rms} peak={levels.mic?.peak} />
        </div>
        <div className="source-row small">
          {settings && (
            <>
              <label className="field inline">
                <span>Preset</span>
                <select value={settings.active_preset} onChange={(e) => update({ active_preset: e.target.value })}>
                  {settings.presets.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="segmented" role="group" aria-label="Gaya bahasa">
                {(["formal", "santai"] as const).map((r) => (
                  <button key={r} className={settings.register === r ? "on" : ""} onClick={() => update({ register: r })} aria-pressed={settings.register === r}>
                    {r === "formal" ? "Formal" : "Santai"}
                  </button>
                ))}
              </div>
              <label className="check">
                <input type="checkbox" checked={settings.auto_assist} onChange={(e) => update({ auto_assist: e.target.checked })} />
                Bantu otomatis saat ada pertanyaan
              </label>
            </>
          )}
          <span className={`status ${sttReady ? "ok" : ""}`}>{sttReady ? "Transkripsi siap" : "Memuat model transkripsi…"}</span>
        </div>
        {notice && (
          <p className="notice" role="status">
            {notice}
            <button className="link" onClick={() => setNotice(null)}>
              Tutup
            </button>
          </p>
        )}
      </section>

      <div className="columns">
        <section className="card transcript" aria-label="Transkrip">
          <div className="card-head">
            <h2>Transkrip</h2>
            <button
              className="ghost"
              onClick={() => {
                api.clearTranscript();
                setLines([]);
              }}
              disabled={lines.length === 0}
            >
              Bersihkan
            </button>
          </div>
          <div className="lines">
            {lines.length === 0 && (
              <p className="empty">Transkrip muncul di sini setelah kamu menekan Mulai dan ada yang bicara.</p>
            )}
            {lines.map((l, i) => (
              <div key={i} className={`line ${l.source}`}>
                <span className="who">{l.source === "mic" ? "Saya" : "Mereka"}</span>
                <span className="ts">{fmtTime(l.start)}</span>
                <p>{l.text}</p>
              </div>
            ))}
            <div ref={endRef} />
          </div>
        </section>

        <section className="card ai" aria-label="Asisten AI">
          <div className="card-head">
            <h2>Asisten</h2>
          </div>
          {question && (
            <p className="question">
              <span>Pertanyaan terdeteksi</span>
              {question}
            </p>
          )}
          <ActionBar assistKey={assistKey} />
          <div className="answers">
            {answers.length === 0 && (
              <p className="empty">
                Jawaban AI muncul di sini. Tekan <b>Bantu jawab</b> atau <kbd>{assistKey}</kbd> kapan saja, termasuk saat
                jendela ini tersembunyi.
              </p>
            )}
            {answers.map((a) => (
              <AnswerCard key={a.id} a={a} />
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
