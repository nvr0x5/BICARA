import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { LangContext, translate, UiLang } from "./i18n";
import { api, applyTheme, Settings } from "./api";
import SettingsView, { SettingsTab } from "./SettingsView";
import StealthView from "./StealthView";
import { IDoc, IEyeOff, IGear, ILock, IMic, ISpark } from "./Icons";

const TABS: [SettingsTab, string, () => React.ReactElement][] = [
  ["umum", "Umum", () => <IGear />],
  ["ai", "AI", () => <ISpark />],
  ["transkripsi", "Transkripsi", () => <IMic />],
  ["preset", "Preset", () => <IDoc />],
  ["siluman", "Siluman", () => <IEyeOff />],
  ["privasi", "Privasi", () => <ILock />],
];

/**
 * Preferences: its own window on macOS (⌘,), a page inside the main window on Windows.
 * Loads settings itself and follows changes made elsewhere.
 */
export default function SettingsWindow({ embedded = false }: { embedded?: boolean }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [tab, setTab] = useState<SettingsTab>("umum");
  const [hotkeys, setHotkeys] = useState({ panic: "", assist: "" });

  useEffect(() => {
    api.getSettings().then(setSettings);
    api.hotkeys().then(setHotkeys);
    const un = listen<Settings>("settings-changed", (e) => setSettings(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, []);
  useEffect(() => {
    if (!embedded) applyTheme(settings?.theme);
  }, [embedded, settings?.theme]);

  const lang: UiLang = settings?.ui_language ?? "id";
  return (
    <LangContext.Provider value={lang}>
      <div className="prefs" lang={lang} style={embedded ? { height: "100%" } : undefined}>
        <nav className="ptabs" role="tablist" data-tauri-drag-region>
          {TABS.map(([id, label, Icon]) => (
            <button key={id} role="tab" aria-selected={tab === id} aria-pressed={tab === id} onClick={() => setTab(id)}>
              <Icon />
              {translate(lang, label)}
            </button>
          ))}
        </nav>
        {settings &&
          (tab === "siluman" ? (
            <div className="scroll">
              <div className="settings">
                <StealthView hotkeys={hotkeys} />
              </div>
            </div>
          ) : (
            <SettingsView settings={settings} onSaved={setSettings} tab={tab} />
          ))}
      </div>
    </LangContext.Provider>
  );
}
