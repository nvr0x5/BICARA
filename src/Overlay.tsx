import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { LangContext, UiLang, useT } from "./i18n";
import { api, applyTheme, lineKey, useAnswers, useLastQuestion, useTranscript } from "./api";
import { AnswerCard } from "./AiPanel";

/** Compact always-on-top panel: last question + latest AI answer. */
/** The overlay is its own window: read the UI language from settings and follow changes. */
export default function Overlay() {
  const [lang, setLang] = useState<UiLang>("id");
  useEffect(() => {
    api.getSettings().then((s) => {
      setLang(s.ui_language ?? "id");
      applyTheme(s.theme);
    });
    const un = listen<{ ui_language?: UiLang; theme?: string }>("settings-changed", (e) => {
      setLang(e.payload.ui_language ?? "id");
      applyTheme(e.payload.theme);
    });
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
  const [lines, , partials, translations] = useTranscript();
  // Live strip: what "Mereka" is saying right now, else their last line (+ translation).
  const lastThem = [...lines].reverse().find((l) => l.source === "app");
  const live = partials.app ?? lastThem;
  const liveTr = live && !live.partial ? translations[lineKey(live)] : undefined;

  return (
    <main className="overlay" data-tauri-drag-region>
      <div className="hbar" data-tauri-drag-region>
        <span className="rec-dot" aria-hidden />
        BICARA
        <span className="spacer" data-tauri-drag-region />
      </div>
      {(live || question) && (
        <div className="hsec live">
          {live ? (
            <>
              <p>{live.text}{live.partial ? "…" : ""}</p>
              {liveTr && <p className="tr">{liveTr}</p>}
            </>
          ) : (
            <p>{question}</p>
          )}
        </div>
      )}
      <div className="hbody">{latest ? <AnswerCard a={latest} /> : <p className="empty">{t("Belum ada jawaban.")}</p>}</div>
      <div className="hkeys">
        <span><b>{keys.assist}</b> {t("bantu jawab")}</span>
        <span><b>{keys.panic}</b> {t("sembunyikan")}</span>
      </div>
    </main>
  );
}
