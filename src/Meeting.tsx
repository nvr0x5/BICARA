import { useCallback, useEffect, useRef, useState } from "react";
import { useT, presetName } from "./i18n";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { api, AudioApp, Doc, fmtTime, lineKey, Meeting as M, Settings, useAnswers, useLastQuestion, useLevels, useTranscript } from "./api";
import { Meter } from "./Meter";
import { ActionBar, AnswerCard } from "./AiPanel";
import { Markdown } from "./Markdown";

type Phase = "setup" | "live" | "ending" | "done";

function useElapsed(since: number | null) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!since) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [since]);
  return since ? Math.max(0, now / 1000 - since) : 0;
}

export default function Meeting({
  settings,
  onSettings,
  assistKey,
  onOpenHistory,
}: {
  settings: Settings | null;
  onSettings: (s: Settings) => void;
  assistKey: string;
  onOpenHistory: (id: number) => void;
}) {
  const t = useT();
  const [phase, setPhase] = useState<Phase>("setup");
  const [draft, setDraft] = useState<M | null>(null);
  const [title, setTitle] = useState("");
  const [presetId, setPresetId] = useState<string>("");
  const [apps, setApps] = useState<AudioApp[]>([]);
  const [appId, setAppId] = useState("");
  const [withMic, setWithMic] = useState(true);
  const [docs, setDocs] = useState<Doc[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [consent, setConsent] = useState(false);
  const [sttReady, setSttReady] = useState(false);
  const [live, setLive] = useState<M | null>(null);
  const [endedId, setEndedId] = useState<number | null>(null);
  const [notulen, setNotulen] = useState<string | null>(null);
  const [prepId, setPrepId] = useState<number | null>(null);
  const [mics, setMics] = useState<string[]>([]);
  const levels = useLevels();
  const [lines, setLines, partials, translations] = useTranscript();
  const answers = useAnswers();
  const question = useLastQuestion();
  const endRef = useRef<HTMLDivElement>(null);
  const elapsed = useElapsed(phase === "live" && live ? live.started_at : null);

  useEffect(() => {
    if (settings && !presetId) setPresetId(settings.active_preset);
  }, [settings, presetId]);

  const refresh = useCallback(async () => {
    try {
      setApps(await api.listAudioApps());
    } catch (e) {
      setNotice(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    api.listMicrophones().then(setMics);
    api.sttLoaded().then(setSttReady);
    api.currentMeeting().then((m) => {
      if (m) {
        setLive(m);
        setPhase("live");
      }
    });
    const uns = [
      listen("stt-ready", () => setSttReady(true)),
      listen<number>("notulen-ready", async (e) => {
        const d = await api.getMeeting(e.payload);
        setNotulen(d.meeting.notulen);
      }),
    ];
    return () => {
      uns.forEach((u) => u.then((f) => f()));
    };
  }, [refresh]);

  useEffect(() => {
    if (phase !== "setup") return;
    const t = setInterval(refresh, 4000);
    return () => clearInterval(t);
  }, [phase, refresh]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [lines.length, partials, translations]);

  const ensureDraft = async (): Promise<M> => {
    if (draft) return draft;
    const m = await api.createMeeting(title, presetId || settings?.active_preset || "rapat");
    setDraft(m);
    return m;
  };

  const addFiles = async () => {
    const exts = await api.docExtensions();
    const picked = await open({ multiple: true, filters: [{ name: t("Dokumen"), extensions: exts }] });
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (!paths.length) return;
    setBusy(t("Membaca dokumen…"));
    try {
      const m = await ensureDraft();
      const res = await api.addMeetingFiles(m.id, paths);
      setDocs((d) => [...d, ...res.flatMap((r) => (r.document ? [r.document] : []))]);
      const errs = res.filter((r) => r.error).map((r) => `${r.name}: ${r.error}`);
      if (errs.length) setNotice(errs.join("\n"));
    } catch (e) {
      setNotice(String(e));
    }
    setBusy(null);
  };

  const removeDoc = async (d: Doc) => {
    await api.removeMeetingDocument(d.id);
    setDocs((x) => x.filter((y) => y.id !== d.id));
  };

  const prep = async () => {
    try {
      const m = await ensureDraft();
      setPrepId(await api.prepareInterview(m.id));
    } catch (e) {
      setNotice(String(e));
    }
  };

  const start = async () => {
    setNotice(null);
    setBusy(t("Menyiapkan…"));
    try {
      const m = await ensureDraft();
      if (title.trim() && title.trim() !== m.title) await api.renameMeeting(m.id, title.trim());
      const app = apps.find((a) => a.id === appId);
      const r = await api.startMeeting(m.id, appId || null, app?.name ?? null, withMic);
      setLines([]);
      setLive(r.meeting);
      setPhase("live");
      if (r.warning) setNotice(r.warning);
      if (r.transcribing) setSttReady(true);
    } catch (e) {
      setNotice(String(e));
    }
    setBusy(null);
  };

  const end = async () => {
    setPhase("ending");
    try {
      const id = await api.endMeeting();
      setEndedId(id);
      setNotulen(null);
    } catch (e) {
      setNotice(String(e));
    }
    setPhase("done");
  };

  const reset = () => {
    setPhase("setup");
    setDraft(null);
    setTitle("");
    setDocs([]);
    setLive(null);
    setEndedId(null);
    setNotulen(null);
    setConsent(false);
    setPrepId(null);
    setLines([]);
    api.clearTranscript();
  };

  const update = (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    onSettings(next);
    api.saveSettings(next).catch((e) => setNotice(String(e)));
  };

  const preset = settings?.presets.find((p) => p.id === presetId);
  const needConsent = settings?.consent_reminder ?? true;
  const prepAnswer = answers.find((a) => a.id === prepId);
  const notulenStream = answers.find((a) => a.action === "notulen");

  // ------------------------------------------------------------------ setup
  if (phase === "setup") {
    return (
      <div className="setup">
        <section className="card">
          <h2>{t("Meeting baru")}</h2>
          <div className="fields">
            <label className="field">
              <span>{t("Judul")}</span>
              <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t("mis. Interview PM — PT Maju")} />
            </label>
            <label className="field">
              <span>{t("Jenis (preset)")}</span>
              <select value={presetId} onChange={(e) => setPresetId(e.target.value)} disabled={!!draft}>
                {settings?.presets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {presetName(t, p)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="source-row">
            <label className="field">
              <span>{t("Aplikasi meeting (audio “Mereka”)")}</span>
              <select value={appId} onChange={(e) => setAppId(e.target.value)}>
                <option value="">{t("— hanya mikrofon —")}</option>
                {apps.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.name}
                    {a.playing ? t(" · memutar audio") : ""}
                  </option>
                ))}
              </select>
            </label>
            <button className="ghost" onClick={refresh}>
              {t("Refresh")}
            </button>
            <label className="check">
              <input type="checkbox" checked={withMic} onChange={(e) => setWithMic(e.target.checked)} />
              {t("Mikrofon saya")}
            </label>
          </div>
          <div className="fields">
            <label className="field">
              <span>{t("Mikrofon (“Saya”)")}</span>
              <select value={settings?.mic_device ?? ""} onChange={(e) => update({ mic_device: e.target.value || null })} disabled={!withMic}>
                <option value="">{t("Default sistem")}</option>
                {mics.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>{t("Bahasa percakapan")}</span>
              <select value={settings?.language ?? "id"} onChange={(e) => update({ language: e.target.value as Settings["language"] })}>
                <option value="id">{t("Bahasa Indonesia")}</option>
                <option value="en">{t("English")}</option>
                <option value="auto">{t("Otomatis (ID / EN)")}</option>
              </select>
            </label>
            <label className="field">
              <span>{t("Terjemahan langsung")}</span>
              <select value={settings?.translate_to ?? ""} onChange={(e) => update({ translate_to: e.target.value as Settings["translate_to"] })}>
                <option value="">{t("Mati")}</option>
                <option value="en">{t("Ke English")}</option>
                <option value="id">{t("Ke Bahasa Indonesia")}</option>
              </select>
            </label>
          </div>
          <p className="hint">{t("Untuk Google Meet pilih browser-nya. Aplikasi baru muncul setelah memakai audio.")}</p>
        </section>

        <section className="card">
          <div className="card-head">
            <h2>{t("Dokumen konteks")}</h2>
            <button className="ghost" onClick={addFiles} disabled={!!busy}>
              {t("Tambah file")}
            </button>
          </div>
          <p className="hint">{t("PDF, Word, PowerPoint, TXT, Markdown — mis. agenda, CV, deskripsi pekerjaan, materi presentasi. AI memakainya saat menjawab.")}</p>
          {docs.length > 0 && (
            <ul className="docs">
              {docs.map((d) => (
                <li key={d.id}>
                  <span>{d.name}</span>
                  <span className="hint">{d.chars < 1000 ? `${d.chars} ${t("karakter")}` : `${Math.round(d.chars / 1000)}${t("rb karakter")}`}</span>
                  <button className="link" onClick={() => removeDoc(d)}>
                    {t("Hapus")}
                  </button>
                </li>
              ))}
            </ul>
          )}
          {presetId === "interview" && (
            <div className="row">
              <button className="ghost" onClick={prep} disabled={docs.length === 0 && !preset?.context}>
                {t("Siapkan jawaban interview")}
              </button>
              <span className="hint">{t("Jawaban STAR untuk 12 pertanyaan HRD umum, dari CV & deskripsi pekerjaan.")}</span>
            </div>
          )}
          {prepAnswer && <AnswerCard a={prepAnswer} />}
        </section>

        <section className="card">
          {needConsent && (
            <label className="check">
              <input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} />
              {t("Peserta sudah saya beri tahu bahwa percakapan ini direkam & ditranskrip (UU PDP).")}
            </label>
          )}
          <div className="row between">
            <span className={`status ${sttReady ? "ok" : ""}`}>{sttReady ? t("Transkripsi siap") : t("Memuat model transkripsi…")}</span>
            <button className="primary big" onClick={start} disabled={!!busy || (!appId && !withMic) || (needConsent && !consent)}>
              {busy ?? t("Mulai meeting")}
            </button>
          </div>
          {notice && <p className="notice" role="status">{t(notice)}</p>}
        </section>
      </div>
    );
  }

  // ------------------------------------------------------------------ done
  if (phase === "done" || phase === "ending") {
    return (
      <div className="setup">
        <section className="card">
          <div className="card-head">
            <h2>{phase === "ending" ? t("Mengakhiri meeting…") : t("Meeting selesai")}</h2>
            <div className="row">
              {endedId && (
                <button className="ghost" onClick={() => onOpenHistory(endedId)}>
                  {t("Buka di riwayat")}
                </button>
              )}
              <button className="primary" onClick={reset}>
                {t("Meeting baru")}
              </button>
            </div>
          </div>
          {notulen ? (
            <Markdown text={notulen} />
          ) : notulenStream ? (
            <AnswerCard a={notulenStream} />
          ) : (
            <p className="thinking">{t("Menyusun notulen…")}</p>
          )}
          {notice && <p className="notice">{t(notice)}</p>}
        </section>
      </div>
    );
  }

  // ------------------------------------------------------------------ live
  const app = apps.find((a) => a.id === appId);
  return (
    <div className="live">
      <section className="card source">
        <div className="source-row">
          <div className="live-title">
            <span className="rec-dot" aria-hidden /> <strong>{live?.title}</strong>
            <span className="hint">{fmtTime(elapsed)}</span>
          </div>
          {settings && (
            <div className="segmented" role="group" aria-label={t("Gaya bahasa")}>
              {(["formal", "santai"] as const).map((r) => (
                <button key={r} className={settings.register === r ? "on" : ""} onClick={() => update({ register: r })} aria-pressed={settings.register === r}>
                  {r === "formal" ? "Formal" : t("Santai")}
                </button>
              ))}
            </div>
          )}
          {settings && (
            <label className="check">
              <input type="checkbox" checked={settings.auto_assist} onChange={(e) => update({ auto_assist: e.target.checked })} />
              {t("Bantu otomatis")}
            </label>
          )}
          <button className="danger" onClick={end}>
            {t("Selesai")}
          </button>
        </div>
        <div className="meters">
          <Meter label={app?.name ?? live?.app_name ?? t("Mereka")} rms={levels.app?.rms} peak={levels.app?.peak} />
          <Meter label={t("Saya")} rms={levels.mic?.rms} peak={levels.mic?.peak} />
        </div>
        {notice && (
          <p className="notice" role="status">
            {t(notice)}
            <button className="link" onClick={() => setNotice(null)}>
              {t("Tutup")}
            </button>
          </p>
        )}
      </section>

      <div className="columns">
        <section className="card transcript" aria-label={t("Transkrip")}>
          <div className="card-head">
            <h2>{t("Transkrip")}</h2>
          </div>
          <div className="lines">
            {lines.length === 0 && <p className="empty">{t("Transkrip muncul di sini begitu ada yang bicara.")}</p>}
            {lines.map((l, i) => (
              <div key={i} className={`line ${l.source === "mic" ? "me" : "them"}`}>
                <span className="who">{l.source === "mic" ? t("Saya") : t("Mereka")}</span>
                <span className="ts">{fmtTime(l.start)}</span>
                <p>{l.text}</p>
                {translations[lineKey(l)] && <p className="tr">{translations[lineKey(l)]}</p>}
              </div>
            ))}
            {Object.values(partials).map((l) => (
              <div key={`p-${l.source}`} className={`line partial ${l.source === "mic" ? "me" : "them"}`} aria-live="off">
                <span className="who">{l.source === "mic" ? t("Saya") : t("Mereka")}</span>
                <span className="ts">{fmtTime(l.start)}</span>
                <p>{l.text}…</p>
              </div>
            ))}
            <div ref={endRef} />
          </div>
        </section>

        <section className="card ai" aria-label={t("Asisten AI")}>
          <div className="card-head">
            <h2>{t("Asisten")}</h2>
          </div>
          {question && (
            <p className="question">
              <span>{t("Pertanyaan terdeteksi")}</span>
              {question}
            </p>
          )}
          <ActionBar assistKey={assistKey} />
          <div className="answers">
            {answers.length === 0 && (
              <p className="empty">
                {t("Tekan {action} atau {key} kapan saja.", { action: "\u0000", key: "\u0001" })
                  .split(/(\u0000|\u0001)/)
                  .map((part, i) => (part === "\u0000" ? <b key={i}>{t("Bantu jawab")}</b> : part === "\u0001" ? <kbd key={i}>{assistKey}</kbd> : part))}
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
