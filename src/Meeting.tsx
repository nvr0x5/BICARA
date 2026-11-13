import { useCallback, useEffect, useRef, useState } from "react";
import { useT, presetName } from "./i18n";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { api, AudioApp, Doc, fmtTime, lineKey, Meeting as M, Settings, useAnswers, useLastQuestion, useLevels, useTranscript } from "./api";
import { Meter } from "./Meter";
import { ActionBar, AnswerCard } from "./AiPanel";
import { IDoc, IInspector, IMic, IMicOff, IRefresh, IStop } from "./Icons";
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
  onLive,
  sidebarToggle,
}: {
  settings: Settings | null;
  onSettings: (s: Settings) => void;
  assistKey: string;
  onOpenHistory: (id: number) => void;
  onLive: (m: M | null) => void;
  sidebarToggle: React.ReactNode;
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
  const [inspector, setInspector] = useState(true);
  const [pinned, setPinned] = useState<number | null>(null);
  const [muted, setMuted] = useState(false);
  const toggleMute = async () => setMuted(await api.setMicMuted(!muted));

  useEffect(() => onLive(phase === "live" ? live : null), [phase, live, onLive]);

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
    // Scroll only the transcript box, never the page (the answer must stay in view).
    const box = endRef.current?.parentElement;
    if (box) box.scrollTop = box.scrollHeight;
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
      setPinned(null);
      setMuted(false);
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

  // Menu bar commands that act on the meeting.
  useEffect(() => {
    const un = listen<string>("menu", (e) => {
      if (e.payload === "toggle-inspector") setInspector((v) => !v);
      if (e.payload === "end" && phase === "live") end();
    });
    return () => {
      un.then((f) => f());
    };
  });

  const bar = (title: React.ReactNode, sub: React.ReactNode, rest?: React.ReactNode) => (
    <header className="toolbar" data-tauri-drag-region>
      {sidebarToggle}
      <div className="title" data-tauri-drag-region>
        <b data-tauri-drag-region>{title}</b>
        {sub && <small data-tauri-drag-region>{sub}</small>}
      </div>
      <div className="spacer" data-tauri-drag-region />
      {rest}
    </header>
  );
  const docSize = (d: Doc) => (d.chars < 1000 ? `${d.chars} ${t("karakter")}` : `${Math.round(d.chars / 1000)}${t("rb karakter")}`);

  // ------------------------------------------------------------------ setup
  if (phase === "setup") {
    return (
      <>
        {bar(t("Meeting baru"), null)}
        <div className="scroll">
          <div className="form">
            <div className="group">
              <h4>{t("Meeting")}</h4>
              <div className="box">
                <label className="item">
                  <span className="lbl">{t("Judul")}</span>
                  <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t("mis. Interview PM — PT Maju")} />
                </label>
                <label className="item">
                  <span className="lbl">{t("Jenis (preset)")}</span>
                  <select value={presetId} onChange={(e) => setPresetId(e.target.value)} disabled={!!draft}>
                    {settings?.presets.map((p) => (
                      <option key={p.id} value={p.id}>
                        {presetName(t, p)}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="item">
                  <span className="lbl">{t("Bahasa percakapan")}</span>
                  <select value={settings?.language ?? "id"} onChange={(e) => update({ language: e.target.value as Settings["language"] })}>
                    <option value="id">{t("Bahasa Indonesia")}</option>
                    <option value="en">{t("English")}</option>
                    <option value="auto">{t("Otomatis (ID / EN)")}</option>
                  </select>
                </label>
                <label className="item">
                  <span className="lbl">{t("Terjemahan langsung")}</span>
                  <select value={settings?.translate_to ?? ""} onChange={(e) => update({ translate_to: e.target.value as Settings["translate_to"] })}>
                    <option value="">{t("Mati")}</option>
                    <option value="en">{t("Ke English")}</option>
                    <option value="id">{t("Ke Bahasa Indonesia")}</option>
                  </select>
                </label>
              </div>
            </div>

            <div className="group">
              <h4>{t("Audio")}</h4>
              <div className="box">
                <div className="item">
                  <label className="lbl" htmlFor="app-src">
                    {t("Aplikasi meeting")}
                    <small>{t("Suara “Mereka”")}</small>
                  </label>
                  <span className="row" style={{ flexWrap: "nowrap", width: "60%", justifyContent: "flex-end" }}>
                    <select id="app-src" value={appId} onChange={(e) => setAppId(e.target.value)} style={{ flex: 1, minWidth: 0 }}>
                      <option value="">{t("— hanya mikrofon —")}</option>
                      {apps.map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.name}
                          {a.playing ? t(" · memutar audio") : ""}
                        </option>
                      ))}
                    </select>
                    <button className="tb" onClick={refresh} aria-label={t("Refresh")} title={t("Refresh")}>
                      <IRefresh />
                    </button>
                  </span>
                </div>
                <label className="item">
                  <span className="lbl">{t("Mikrofon saya")}<small>{t("Suara “Saya”")}</small></span>
                  <input type="checkbox" className="switch" checked={withMic} onChange={(e) => setWithMic(e.target.checked)} />
                </label>
                {withMic && (
                  <label className="item">
                    <span className="lbl">{t("Mikrofon")}</span>
                    <select value={settings?.mic_device ?? ""} onChange={(e) => update({ mic_device: e.target.value || null })}>
                      <option value="">{t("Default sistem")}</option>
                      {mics.map((m) => (
                        <option key={m} value={m}>
                          {m}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
              </div>
              <p className="hint">{t("Untuk Google Meet pilih browser-nya. Aplikasi baru muncul setelah memakai audio.")}</p>
            </div>

            <div className="group">
              <h4>{t("Dokumen konteks")}</h4>
              <div className="box">
                {docs.map((d) => (
                  <div key={d.id} className="item file">
                    <IDoc className="ico" />
                    <span className="name">
                      {d.name}
                      <small>{docSize(d)}</small>
                    </span>
                    <button className="push" onClick={() => removeDoc(d)}>
                      {t("Hapus")}
                    </button>
                  </div>
                ))}
                <div className="item">
                  <button className="push" onClick={addFiles} disabled={!!busy}>
                    {t("Tambah file…")}
                  </button>
                  <span className="hint">{t("PDF, Word, PowerPoint, teks")}</span>
                </div>
                {presetId === "interview" && (
                  <div className="item">
                    <span className="lbl">
                      {t("Siapkan jawaban interview")}
                      <small>{t("Jawaban STAR untuk 12 pertanyaan HRD umum, dari CV & deskripsi pekerjaan.")}</small>
                    </span>
                    <button className="push" onClick={prep} disabled={docs.length === 0 && !preset?.context}>
                      {t("Siapkan")}
                    </button>
                  </div>
                )}
                {prepAnswer && (
                  <div className="item stack">
                    <AnswerCard a={prepAnswer} />
                  </div>
                )}
              </div>
              <p className="hint">{t("AI memakai dokumen ini saat menjawab, mis. agenda, CV, deskripsi pekerjaan.")}</p>
            </div>

            {needConsent && (
              <div className="group">
                <div className="box">
                  <label className="item">
                    <span className="lbl">
                      {t("Peserta sudah diberi tahu bahwa meeting direkam")}
                      <small>{t("UU PDP")}</small>
                    </span>
                    <input type="checkbox" className="switch" checked={consent} onChange={(e) => setConsent(e.target.checked)} />
                  </label>
                </div>
              </div>
            )}

            {notice && <p className="notice" role="status">{t(notice)}</p>}
            <div className="foot">
              <span className={`status ${sttReady ? "ok" : ""}`}>{sttReady ? t("Transkripsi siap") : t("Memuat model transkripsi…")}</span>
              <button className="primary big" onClick={start} disabled={!!busy || (!appId && !withMic) || (needConsent && !consent)}>
                {busy ?? t("Mulai meeting")}
              </button>
            </div>
          </div>
        </div>
      </>
    );
  }

  // ------------------------------------------------------------------ done
  if (phase === "done" || phase === "ending") {
    return (
      <>
        {bar(
          phase === "ending" ? t("Mengakhiri meeting…") : t("Meeting selesai"),
          live?.title,
          <>
            {endedId && (
              <button className="push" onClick={() => onOpenHistory(endedId)}>
                {t("Buka di riwayat")}
              </button>
            )}
            <button className="primary" onClick={reset}>
              {t("Meeting baru")}
            </button>
          </>,
        )}
        <div className="scroll">
          <article className="notes">
            {notulen ? <Markdown text={notulen} /> : notulenStream ? <AnswerCard a={notulenStream} /> : <p className="thinking">{t("Menyusun notulen…")}</p>}
            {notice && <p className="notice">{t(notice)}</p>}
          </article>
        </div>
      </>
    );
  }

  // ------------------------------------------------------------------ live: transcript document + assistant inspector
  const app = apps.find((a) => a.id === appId);
  const shown = answers.find((a) => a.id === pinned) ?? answers[0];
  const earlier = answers.filter((a) => a !== shown).slice(0, 8);
  const askedLine = question ? [...lines].reverse().find((l) => l.source === "app" && l.text === question) : undefined;
  return (
    <>
      {bar(
        live?.title,
        <>
          {app?.name ?? live?.app_name ?? t("Mikrofon")} · {fmtTime(elapsed)}
        </>,
        <>
          {settings && (
            <span className="seg" role="group" aria-label={t("Gaya bahasa")}>
              {(["formal", "santai"] as const).map((r) => (
                <button key={r} onClick={() => update({ register: r })} aria-pressed={settings.register === r}>
                  {r === "formal" ? "Formal" : t("Santai")}
                </button>
              ))}
            </span>
          )}
          <button className="tb rec" onClick={end} title={t("Akhiri meeting")}>
            <IStop />
            {t("Akhiri")}
          </button>
          <button className="tb" onClick={() => setInspector((v) => !v)} aria-pressed={inspector} aria-label={t("Tampilkan/sembunyikan asisten")}>
            <IInspector />
          </button>
        </>,
      )}
      <div className={`livebody ${inspector ? "" : "no-inspector"}`}>
        <article className="doc" aria-label={t("Transkrip")}>
          <p className="day">
            {t("Transkrip")}
            {settings?.translate_to ? ` · ${t("terjemahan aktif")}` : ""}
          </p>
          {lines.length === 0 && Object.keys(partials).length === 0 && <p className="empty">{t("Transkrip muncul di sini begitu ada yang bicara.")}</p>}
          {lines.map((l, i) => (
            <div key={i} className={`utt ${l.source === "mic" ? "me" : ""} ${l === askedLine ? "asked" : ""}`}>
              <div className="who">
                {l.source === "mic" ? t("Saya") : t("Mereka")}
                <small>{fmtTime(l.start)}</small>
              </div>
              <div>
                <p>{l.text}</p>
                {translations[lineKey(l)] && <p className="tr">{translations[lineKey(l)]}</p>}
              </div>
            </div>
          ))}
          {Object.values(partials).map((l) => (
            <div key={`p-${l.source}`} className={`utt partial ${l.source === "mic" ? "me" : ""}`} aria-live="off">
              <div className="who">
                {l.source === "mic" ? t("Saya") : t("Mereka")}
                <small>{fmtTime(l.start)}</small>
              </div>
              <div>
                <p>{l.text}…</p>
              </div>
            </div>
          ))}
          <div ref={endRef} />
        </article>
        {inspector && (
          <aside className="inspector" aria-label={t("Asisten AI")}>
            <section className="insp">
              <h5>
                {t("Pertanyaan")}
                {askedLine && <span>{fmtTime(askedLine.start)}</span>}
              </h5>
              {question ? <p className="asked">{question}</p> : <p className="empty">{t("Belum ada pertanyaan terdeteksi.")}</p>}
            </section>
            <section className="insp" aria-live="polite">
              {shown ? (
                <AnswerCard a={shown} />
              ) : (
                <p className="empty">
                  {t("Tekan {action} atau {key} kapan saja.", { action: "\u0000", key: "\u0001" })
                    .split(/(\u0000|\u0001)/)
                    .map((part, i) => (part === "\u0000" ? <b key={i}>{t("Bantu jawab")}</b> : part === "\u0001" ? <kbd key={i}>{assistKey}</kbd> : part))}
                </p>
              )}
            </section>
            <section className="insp">
              <ActionBar assistKey={assistKey} />
            </section>
            {earlier.length > 0 && (
              <section className="insp">
                <h5>{t("Sebelumnya")}</h5>
                <ul className="older">
                  {earlier.map((a) => (
                    <li key={a.id}>
                      <button onClick={() => setPinned(a.id)}>
                        {(a.text || a.title).replace(/[*#_>`]/g, "").slice(0, 90)}
                        <small>{t(a.title)}</small>
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            )}
          </aside>
        )}
      </div>
      <footer className="statusbar">
        <span className={sttReady ? "ok" : "wait"}>{sttReady ? t("Transkripsi lokal (whisper)") : t("Memuat model transkripsi…")}</span>
        <span className="lvl">
          {t("Mereka")}
          <Meter rms={levels.app?.rms} peak={levels.app?.peak} />
        </span>
        <span className="lvl">
          {t("Saya")}
          <Meter rms={muted ? 0 : levels.mic?.rms} peak={muted ? 0 : levels.mic?.peak} />
          <button
            className={`tb mute ${muted ? "on" : ""}`}
            onClick={toggleMute}
            aria-pressed={muted}
            title={muted ? t("Mikrofon dibisukan — suara kamu tidak direkam") : t("Bisukan mikrofon")}
          >
            {muted ? <IMicOff /> : <IMic />}
            {muted ? t("Dibisukan") : t("Bisukan")}
          </button>
        </span>
        {settings && (
          <label className="check">
            <input type="checkbox" checked={settings.auto_assist} onChange={(e) => update({ auto_assist: e.target.checked })} />
            {t("Bantu otomatis")}
          </label>
        )}
        {notice && (
          <span className="notice" role="status">
            {t(notice)}{" "}
            <button className="link" onClick={() => setNotice(null)}>
              {t("Tutup")}
            </button>
          </span>
        )}
        <span className="right">{t("{n} dokumen konteks", { n: String(docs.length) })}</span>
      </footer>
    </>
  );
}
