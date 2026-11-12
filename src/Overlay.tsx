import { useEffect, useState } from "react";
import { api, useAnswers, useLastQuestion } from "./api";
import { ActionBar, AnswerCard } from "./AiPanel";

/** Compact always-on-top panel: last question + latest AI answer. */
export default function Overlay() {
  const answers = useAnswers();
  const question = useLastQuestion();
  const [keys, setKeys] = useState({ panic: "", assist: "" });
  useEffect(() => {
    api.hotkeys().then(setKeys);
  }, []);
  const latest = answers[0];

  return (
    <main className="overlay">
      {question && (
        <p className="question">
          <span>Pertanyaan</span>
          {question}
        </p>
      )}
      <div className="overlay-body">
        {latest ? (
          <AnswerCard a={latest} />
        ) : (
          <p className="empty">
            <kbd>{keys.assist}</kbd> bantu jawab · <kbd>{keys.panic}</kbd> sembunyikan
          </p>
        )}
      </div>
      <ActionBar compact />
    </main>
  );
}
