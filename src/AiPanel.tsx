import { useState } from "react";
import { useT } from "./i18n";
import { AiAnswer, api } from "./api";
import { Markdown } from "./Markdown";

export function AnswerCard({ a }: { a: AiAnswer }) {
  const t = useT();
  return (
    <article className={`answer ${a.action}`} aria-busy={!a.done}>
      <header>
        <span className="answer-title">{a.title.startsWith("Notulen · ") ? `${t("Notulen")} · ${a.title.slice(10)}` : t(a.title)}</span>
        <span className="answer-meta">
          {a.provider.replace("(langganan)", t("(langganan)"))}
          {a.ms != null && ` · ${(a.ms / 1000).toFixed(1)} ${t("dtk")}`}
        </span>
      </header>
      {a.error ? (
        <p className="answer-error">{t(a.error)}</p>
      ) : a.text ? (
        <Markdown text={a.text} />
      ) : (
        <p className="thinking">{t("Sedang berpikir…")}</p>
      )}
    </article>
  );
}

export function ActionBar({ assistKey, compact }: { assistKey?: string; compact?: boolean }) {
  const t = useT();
  const [q, setQ] = useState("");
  const ask = () => {
    if (!q.trim()) return;
    api.aiAction("ask", q);
    setQ("");
  };
  return (
    <div className={`actionbar ${compact ? "compact" : ""}`}>
      <div className="actions">
        <button className="primary" onClick={() => api.aiAction("assist")} title={assistKey ? t("Pintasan {key}", { key: assistKey }) : undefined}>
          {t("Bantu jawab")}{assistKey && !compact ? <kbd>{assistKey}</kbd> : null}
        </button>
        <button className="ghost" onClick={() => api.aiAction("followup")}>
          {t("Pertanyaan lanjutan")}
        </button>
        <button className="ghost" onClick={() => api.aiAction("summary")}>
          {t("Ringkas")}
        </button>
      </div>
      <form
        className="ask"
        onSubmit={(e) => {
          e.preventDefault();
          ask();
        }}
      >
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder={t("Tanya AI soal percakapan ini…")} aria-label={t("Pertanyaan untuk AI")} />
        <button className="ghost" type="submit" disabled={!q.trim()}>
          {t("Tanya")}
        </button>
      </form>
    </div>
  );
}
