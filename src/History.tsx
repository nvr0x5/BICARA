import { useEffect, useState } from "react";
import { useT } from "./i18n";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, fmtDate, fmtTime, Meeting, MeetingDetail, SearchHit, useAnswers } from "./api";
import { Markdown } from "./Markdown";
import { AnswerCard } from "./AiPanel";

export default function History({ openId }: { openId: number | null }) {
  const t = useT();
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [sel, setSel] = useState<number | null>(openId);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [view, setView] = useState<"notulen" | "transkrip" | "ai">("notulen");
  const [ask, setAsk] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const answers = useAnswers();

  const reload = () => api.listMeetings().then(setMeetings);
  useEffect(() => {
    reload();
    const un = listen<number>("notulen-ready", (e) => {
      reload();
      if (e.payload === sel) api.getMeeting(e.payload).then(setDetail);
    });
    return () => {
      un.then((f) => f());
    };
  }, [sel]);
  useEffect(() => setSel(openId), [openId]);
  useEffect(() => {
    setDetail(null);
    setEditing(null);
    if (sel != null) api.getMeeting(sel).then(setDetail).catch((e) => setNotice(String(e)));
  }, [sel]);

  useEffect(() => {
    if (!query.trim()) {
      setHits(null);
      return;
    }
    const t = setTimeout(() => api.searchMeetings(query).then(setHits), 250);
    return () => clearTimeout(t);
  }, [query]);

  const m = detail?.meeting;
  const slug = (m?.title ?? "meeting").replace(/[^\p{L}\p{N}]+/gu, "-").toLowerCase();

  const exportAs = async (ext: "docx" | "md") => {
    if (!m) return;
    const path = await save({ defaultPath: `${slug}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!path) return;
    try {
      const p = await api.exportMeeting(m.id, path);
      setNotice(t("Tersimpan: {path}", { path: p }));
      revealItemInDir(p);
    } catch (e) {
      setNotice(String(e));
    }
  };

  const share = async () => {
    if (!m) return;
    try {
      await openUrl(await api.whatsappUrl(m.id));
    } catch (e) {
      setNotice(String(e));
    }
  };

  const del = async () => {
    if (!m || !confirm(t('Hapus "{title}" beserta transkrip, notulen, dokumen, dan rekamannya? Tidak bisa dibatalkan.', { title: m.title }))) return;
    try {
      await api.deleteMeeting(m.id);
      setSel(null);
      reload();
    } catch (e) {
      setNotice(String(e));
    }
  };

  const meetingAnswers = answers.filter((a) => a.action === "ask" && a.title === "Tanya meeting");

  return (
    <div className="history">
      <aside className="card list">
        <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Cari di semua meeting…")} aria-label={t("Cari meeting")} />
        <ul>
          {hits
            ? hits.map((h) => (
                <li key={h.meeting_id}>
                  <button className={sel === h.meeting_id ? "on" : ""} onClick={() => setSel(h.meeting_id)}>
                    <strong>{h.title}</strong>
                    <span className="hint">{fmtDate(h.started_at)}</span>
                    <span className="snippet">{h.snippet}</span>
                  </button>
                </li>
              ))
            : meetings.map((x) => (
                <li key={x.id}>
                  <button className={sel === x.id ? "on" : ""} onClick={() => setSel(x.id)}>
                    <strong>{x.title}</strong>
                    <span className="hint">
                      {fmtDate(x.started_at)} · {x.utterances} {t("kalimat")}{x.ended_at ? "" : t(" · belum selesai")}
                    </span>
                  </button>
                </li>
              ))}
          {!hits && meetings.length === 0 && <li className="empty">{t("Belum ada meeting.")}</li>}
          {hits && hits.length === 0 && <li className="empty">{t("Tidak ditemukan.")}</li>}
        </ul>
      </aside>

      <section className="card detail">
        {!m ? (
          <p className="empty">{t("Pilih meeting di kiri.")}</p>
        ) : (
          <>
            <div className="card-head">
              <div>
                <h2>{m.title}</h2>
                <span className="hint">
                  {detail!.date}
                  {m.app_name ? ` · ${m.app_name}` : ""}
                  {m.ended_at ? ` · ${Math.round((m.ended_at - m.started_at) / 60)} ${t("menit")}` : ""}
                </span>
              </div>
              <div className="row">
                <button className="ghost" onClick={() => exportAs("docx")}>Word</button>
                <button className="ghost" onClick={() => exportAs("md")}>Markdown</button>
                <button className="ghost" onClick={share} disabled={!m.notulen}>WhatsApp</button>
                <button className="ghost danger-text" onClick={del}>{t("Hapus")}</button>
              </div>
            </div>
            {notice && (
              <p className="hint" role="status">
                {t(notice)}
              </p>
            )}
            {detail!.documents.length > 0 && <p className="hint">{t("Dokumen:")} {detail!.documents.map((d) => d.name).join(", ")}</p>}
            <nav className="tabs small" role="tablist">
              {(
                [
                  ["notulen", t("Notulen")],
                  ["transkrip", `${t("Transkrip")} (${detail!.utterances.length})`],
                  ["ai", `${t("Jawaban AI")} (${detail!.answers.length})`],
                ] as const
              ).map(([id, label]) => (
                <button key={id} role="tab" aria-selected={view === id} className={view === id ? "on" : ""} onClick={() => setView(id)}>
                  {label}
                </button>
              ))}
            </nav>

            <div className="detail-body">
              {view === "notulen" &&
                (editing != null ? (
                  <div className="grid-gap">
                    <textarea rows={18} value={editing} onChange={(e) => setEditing(e.target.value)} />
                    <div className="row">
                      <button
                        className="primary"
                        onClick={async () => {
                          await api.saveNotulen(m.id, editing);
                          setEditing(null);
                          setDetail(await api.getMeeting(m.id));
                        }}
                      >
                        {t("Simpan")}
                      </button>
                      <button className="ghost" onClick={() => setEditing(null)}>{t("Batal")}</button>
                    </div>
                  </div>
                ) : (
                  <div className="grid-gap">
                    {m.notulen ? <Markdown text={m.notulen} /> : <p className="empty">{t("Belum ada notulen.")}</p>}
                    <div className="row">
                      <button className="ghost" onClick={() => setEditing(m.notulen ?? "")}>{t("Edit")}</button>
                      <button className="ghost" onClick={() => api.generateNotulen(m.id)} disabled={detail!.utterances.length === 0}>
                        {t("Buat ulang notulen")}
                      </button>
                    </div>
                    {answers.find((a) => a.action === "notulen" && !a.done) && <AnswerCard a={answers.find((a) => a.action === "notulen" && !a.done)!} />}
                  </div>
                ))}
              {view === "transkrip" && (
                <div className="lines">
                  {detail!.utterances.map((l, i) => (
                    <div key={i} className={`line ${l.source === "mic" ? "me" : "them"}`}>
                      <span className="who">{l.source === "mic" ? t("Saya") : t("Mereka")}</span>
                      <span className="ts">{fmtTime(l.start)}</span>
                      <p>{l.text}</p>
                    </div>
                  ))}
                  {m.recording_dir && (
                    <button className="link" onClick={() => revealItemInDir(m.recording_dir!)}>
                      {t("Buka folder rekaman")}
                    </button>
                  )}
                </div>
              )}
              {view === "ai" && (
                <div className="answers">
                  {detail!.answers.map((a) => (
                    <article key={a.id} className={`answer ${a.action}`}>
                      <header>
                        <span className="answer-title">{a.title}</span>
                        <span className="answer-meta">{fmtDate(a.created_at)}</span>
                      </header>
                      {a.input && <p className="hint">«{a.input}»</p>}
                      <Markdown text={a.text} />
                    </article>
                  ))}
                </div>
              )}
            </div>

            <form
              className="ask"
              onSubmit={(e) => {
                e.preventDefault();
                if (!ask.trim()) return;
                api.askMeeting(m.id, ask).catch((err) => setNotice(String(err)));
                setAsk("");
              }}
            >
              <input value={ask} onChange={(e) => setAsk(e.target.value)} placeholder={t("Tanya tentang meeting ini, mis. “Apa tenggat yang disepakati?”")} aria-label={t("Tanya tentang meeting ini")} />
              <button className="ghost" type="submit" disabled={!ask.trim()}>
                {t("Tanya")}
              </button>
            </form>
            {meetingAnswers.slice(0, 3).map((a) => (
              <AnswerCard key={a.id} a={a} />
            ))}
          </>
        )}
      </section>
    </div>
  );
}
