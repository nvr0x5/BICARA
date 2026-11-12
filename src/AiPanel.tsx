import { useState } from "react";
import { AiAnswer, api } from "./api";
import { Markdown } from "./Markdown";

export function AnswerCard({ a }: { a: AiAnswer }) {
  return (
    <article className={`answer ${a.action}`} aria-busy={!a.done}>
      <header>
        <span className="answer-title">{a.title}</span>
        <span className="answer-meta">
          {a.provider}
          {a.ms != null && ` · ${(a.ms / 1000).toFixed(1)} dtk`}
        </span>
      </header>
      {a.error ? (
        <p className="answer-error">{a.error}</p>
      ) : a.text ? (
        <Markdown text={a.text} />
      ) : (
        <p className="thinking">Sedang berpikir…</p>
      )}
    </article>
  );
}

export function ActionBar({ assistKey, compact }: { assistKey?: string; compact?: boolean }) {
  const [q, setQ] = useState("");
  const ask = () => {
    if (!q.trim()) return;
    api.aiAction("ask", q);
    setQ("");
  };
  return (
    <div className={`actionbar ${compact ? "compact" : ""}`}>
      <div className="actions">
        <button className="primary" onClick={() => api.aiAction("assist")} title={assistKey ? `Pintasan ${assistKey}` : undefined}>
          Bantu jawab{assistKey && !compact ? <kbd>{assistKey}</kbd> : null}
        </button>
        <button className="ghost" onClick={() => api.aiAction("followup")}>
          Pertanyaan lanjutan
        </button>
        <button className="ghost" onClick={() => api.aiAction("summary")}>
          Ringkas
        </button>
      </div>
      <form
        className="ask"
        onSubmit={(e) => {
          e.preventDefault();
          ask();
        }}
      >
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Tanya AI soal percakapan ini…" aria-label="Pertanyaan untuk AI" />
        <button className="ghost" type="submit" disabled={!q.trim()}>
          Tanya
        </button>
      </form>
    </div>
  );
}
