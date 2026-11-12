import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { LangContext, UiLang, useT } from "./i18n";
import { api, useAnswers, useLastQuestion } from "./api";
import { ActionBar, AnswerCard } from "./AiPanel";

/** Compact always-on-top panel: last question + latest AI answer. */
/** The overlay is its own window: read the UI language from settings and follow changes. */
export default function Overlay() {
  const [lang, setLang] = useState<UiLang>("id");
  useEffect(() => {
    api.getSettings().then((s) => setLang(s.ui_language ?? "id"));
    const un = listen<{ ui_language?: UiLang }>("settings-changed", (e) => setLang(e.payload.ui_language ?? "id"));
    return () => {
      un.then((f) => f());
    };
  }, []);
  return (
    <LangContext.Provider value={lang}>
      <OverlayBody />
    </LangContext.Provider>
  );
}

function OverlayBody() {
  const t = useT();
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
          <span>{t("Pertanyaan")}</span>
          {question}
        </p>
      )}
      <div className="overlay-body">
        {latest ? (
          <AnswerCard a={latest} />
        ) : (
          <p className="empty">
            <kbd>{keys.assist}</kbd> {t("bantu jawab")} · <kbd>{keys.panic}</kbd> {t("sembunyikan")}
          </p>
        )}
      </div>
      <ActionBar compact />
    </main>
  );
}
