import { useEffect, useState } from "react";
import { useT } from "./i18n";
import { listen } from "@tauri-apps/api/event";
import { ask as askDialog, save } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, fmtDate, fmtShort, fmtTime, Meeting, MeetingDetail, SearchHit, useAnswers } from "./api";
import { Markdown } from "./Markdown";
import { AnswerCard } from "./AiPanel";
import { IPencil, IShare, ITrash } from "./Icons";

type Tab = "notulen" | "transkrip" | "ai";

/** Mail-style history: meeting list in the middle, notes on the right. */
export default function History({ openId, onDeleted }: { openId: number | null; onDeleted: () => void }) {
  const t = useT();
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [sel, setSel] = useState<number | null>(openId);
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [tab, setTab] = useState<Tab>("notulen");
  const [ask, setAsk] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [shareOpen, setShareOpen] = useState(false);
  const answers = useAnswers();

  const reload = () =>
    api.listMeetings().then((ms) => {
      setMeetings(ms);
      setSel((s) => s ?? ms[0]?.id ?? null);
    });
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
  useEffect(() => {
    if (openId != null) setSel(openId);
  }, [openId]);
  useEffect(() => {
    setDetail(null);
    setEditing(null);
    setNotice(null);
    if (sel != null) api.getMeeting(sel).then(setDetail).catch((e) => setNotice(String(e)));
  }, [sel]);
  useEffect(() => {
    if (!query.trim()) return setHits(null);
    const h = setTimeout(() => api.searchMeetings(query).then(setHits), 250);
    return () => clearTimeout(h);
  }, [query]);

  const m = detail?.meeting;
  const slug = (m?.title ?? "meeting").replace(/[^\p{L}\p{N}]+/gu, "-").toLowerCase();

  const exportAs = async (ext: "docx" | "md") => {
    setShareOpen(false);
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
  const whatsapp = async () => {
    setShareOpen(false);
    if (!m) return;
    try {
      await openUrl(await api.whatsappUrl(m.id));
    } catch (e) {
      setNotice(String(e));
    }
  };
  const del = async () => {
    if (!m) return;
    // Native dialog: window.confirm() does not block inside the app's webview, so it can't be trusted.
    const ok = await askDialog(t('Hapus "{title}" beserta transkrip, notulen, dokumen, dan rekamannya? Tidak bisa dibatalkan.', { title: m.title }), {
      title: t("Hapus meeting?"),
      kind: "warning",
      okLabel: t("Hapus"),
      cancelLabel: t("Batal"),
    });
    if (!ok) return;
    try {
      await api.deleteMeeting(m.id);
      setSel(null);
      setDetail(null);
      reload();
      onDeleted();
    } catch (e) {
      setNotice(String(e));
    }
  };

  const meetingAnswers = answers.filter((a) => a.action === "ask" && a.title === "Tanya meeting");
  const notulenStream = answers.find((a) => a.action === "notulen" && !a.done);
  const mins = m?.ended_at ? Math.round((m.ended_at - m.started_at) / 60) : null;

  return (
    <>
      <header className="toolbar" data-tauri-drag-region>
        <div className="title" data-tauri-drag-region>
          <b data-tauri-drag-region>{m?.title ?? t("Riwayat")}</b>
          {m && (
            <small data-tauri-drag-region>
              {detail!.date}
              {mins != null ? ` · ${mins} ${t("menit")}` : ""}
            </small>
          )}
        </div>
        <div className="spacer" data-tauri-drag-region />
        {m && (
          <>
            <span className="seg" role="tablist">
              {(
                [
                  ["notulen", t("Notulen")],
                  ["transkrip", t("Transkrip")],
                  ["ai", "AI"],
                ] as const
              ).map(([id, label]) => (
                <button key={id} role="tab" aria-selected={tab === id} aria-pressed={tab === id} onClick={() => setTab(id)}>
                  {label}
                </button>
              ))}
            </span>
            <button className="tb" onClick={() => setEditing(m.notulen ?? "")} disabled={tab !== "notulen"} title={t("Edit notulen")} aria-label={t("Edit notulen")}>
              <IPencil />
            </button>
            <span style={{ position: "relative" }}>
              <button className="tb" onClick={() => setShareOpen((v) => !v)} aria-expanded={shareOpen} title={t("Bagikan")} aria-label={t("Bagikan")}>
                <IShare />
              </button>
              {shareOpen && (
                <div className="menu" role="menu" onMouseLeave={() => setShareOpen(false)}>
                  <button role="menuitem" onClick={() => exportAs("docx")}>{t("Simpan sebagai Word…")}</button>
                  <button role="menuitem" onClick={() => exportAs("md")}>{t("Simpan sebagai Markdown…")}</button>
                  <button role="menuitem" onClick={whatsapp} disabled={!m.notulen}>{t("Kirim lewat WhatsApp")}</button>
                </div>
              )}
            </span>
            <button className="tb" onClick={del} title={t("Hapus")} aria-label={t("Hapus")}>
              <ITrash />
            </button>
          </>
        )}
      </header>

      <div className="split">
        <aside className="mlist">
          <div className="search">
            <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Cari di semua meeting")} aria-label={t("Cari meeting")} />
          </div>
          <ul>
            {(hits
              ? hits.map((h) => ({ id: h.meeting_id, title: h.title, when: h.started_at, snip: h.snippet }))
              : meetings.map((x) => ({
                  id: x.id,
                  title: x.title,
                  when: x.started_at,
                  snip: x.notulen?.replace(/[#*_>`-]/g, " ").replace(/\s+/g, " ").trim().slice(0, 160) || `${x.utterances} ${t("kalimat")}${x.ended_at ? "" : t(" · belum selesai")}`,
                }))
            ).map((r) => (
              <li key={r.id}>
                <button className={sel === r.id ? "on" : ""} onClick={() => setSel(r.id)}>
                  <span className="meta">
                    <b>{r.title}</b>
                    <span style={{ flex: "none" }}>{fmtShort(r.when)}</span>
                  </span>
                  <span className="snip">{r.snip}</span>
                </button>
              </li>
            ))}
            {!hits && meetings.length === 0 && <li className="empty">{t("Belum ada meeting.")}</li>}
            {hits && hits.length === 0 && <li className="empty">{t("Tidak ditemukan.")}</li>}
          </ul>
        </aside>

        <section className="detail">
          {!m ? (
            <p className="empty" style={{ padding: 28 }}>{t("Pilih meeting di kiri.")}</p>
          ) : (
            <>
              <div className="scroll">
                <article className="notes">
                  <h2>{m.title}</h2>
                  <p className="sub">
                    {[m.app_name, detail!.documents.length ? `${t("Dokumen:")} ${detail!.documents.map((d) => d.name).join(", ")}` : null].filter(Boolean).join(" · ") ||
                      detail!.date}
                  </p>
                  {notice && <p className="hint" role="status">{t(notice)}</p>}

                  {tab === "notulen" &&
                    (editing != null ? (
                      <div style={{ display: "grid", gap: 8 }}>
                        <textarea rows={20} value={editing} onChange={(e) => setEditing(e.target.value)} />
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
                          <button className="push" onClick={() => setEditing(null)}>{t("Batal")}</button>
                        </div>
                      </div>
                    ) : (
                      <>
                        {m.notulen ? <Markdown text={m.notulen} /> : <p className="empty">{t("Belum ada notulen.")}</p>}
                        {notulenStream && <AnswerCard a={notulenStream} />}
                        <p style={{ marginTop: 16 }}>
                          <button className="push" onClick={() => api.generateNotulen(m.id)} disabled={detail!.utterances.length === 0}>
                            {t("Buat ulang notulen")}
                          </button>
                        </p>
                      </>
                    ))}

                  {tab === "transkrip" && (
                    <>
                      {detail!.utterances.map((l, i) => (
                        <div key={i} className={`utt ${l.source === "mic" ? "me" : ""}`}>
                          <div className="who">
                            {l.source === "mic" ? t("Saya") : t("Mereka")}
                            <small>{fmtTime(l.start)}</small>
                          </div>
                          <div>
                            <p>{l.text}</p>
                            {l.translation && <p className="tr">{l.translation}</p>}
                          </div>
                        </div>
                      ))}
                      {m.recording_dir && (
                        <button className="link" onClick={() => revealItemInDir(m.recording_dir!)}>
                          {t("Buka folder rekaman")}
                        </button>
                      )}
                    </>
                  )}

                  {tab === "ai" &&
                    (detail!.answers.length === 0 ? (
                      <p className="empty">{t("Belum ada jawaban AI.")}</p>
                    ) : (
                      detail!.answers.map((a) => (
                        <div key={a.id} className="qa">
                          <div className="answer">
                            <header>
                              <span className="answer-title">{t(a.title)}</span>
                              <span className="answer-meta">{fmtDate(a.created_at)}</span>
                            </header>
                            {a.input && <q>{a.input}</q>}
                            <Markdown text={a.text} />
                          </div>
                        </div>
                      ))
                    ))}
                </article>
              </div>
              <form
                className="askbar"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (!ask.trim()) return;
                  api.askMeeting(m.id, ask).catch((err) => setNotice(String(err)));
                  setAsk("");
                }}
              >
                {meetingAnswers[0] && <AnswerCard a={meetingAnswers[0]} />}
                <div className="ask">
                  <input value={ask} onChange={(e) => setAsk(e.target.value)} placeholder={t("Tanya tentang meeting ini, mis. “Apa tenggat yang disepakati?”")} aria-label={t("Tanya tentang meeting ini")} />
                  <button className="push" type="submit" disabled={!ask.trim()}>
                    {t("Tanya")}
                  </button>
                </div>
              </form>
            </>
          )}
        </section>
      </div>
    </>
  );
}
