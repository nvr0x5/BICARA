import { useEffect, useState } from "react";
import { LangContext, translate } from "./i18n";
import { api, Settings } from "./api";
import Meeting from "./Meeting";
import History from "./History";
import SettingsView from "./SettingsView";
import StealthView from "./StealthView";

type Tab = "live" | "history" | "settings" | "stealth";
const TABS: [Tab, string][] = [
  ["live", "Meeting"],
  ["history", "Riwayat"],
  ["settings", "Pengaturan"],
  ["stealth", "Siluman"],
];

export default function App() {
  const [tab, setTab] = useState<Tab>("live");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [hotkeys, setHotkeys] = useState({ panic: "", assist: "" });
  const [openId, setOpenId] = useState<number | null>(null);

  useEffect(() => {
    api.getSettings().then(setSettings);
    api.hotkeys().then(setHotkeys);
  }, []);

  const lang = settings?.ui_language ?? "id";
  return (
    <LangContext.Provider value={lang}>
    <main className="app" lang={lang}>
      <header className="top">
        <h1>BICARA</h1>
        <nav className="tabs" role="tablist">
          {TABS.map(([id, label]) => (
            <button key={id} role="tab" aria-selected={tab === id} className={tab === id ? "on" : ""} onClick={() => setTab(id)}>
              {translate(lang, label)}
            </button>
          ))}
        </nav>
      </header>
      {/* Live stays mounted so recording state and streams survive tab switches. */}
      <div hidden={tab !== "live"}>
        <Meeting
          settings={settings}
          onSettings={setSettings}
          assistKey={hotkeys.assist}
          onOpenHistory={(id) => {
            setOpenId(id);
            setTab("history");
          }}
        />
      </div>
      {tab === "history" && <History openId={openId} />}
      {tab === "settings" && settings && <SettingsView settings={settings} onSaved={setSettings} />}
      {tab === "stealth" && <StealthView hotkeys={hotkeys} />}
    </main>
    </LangContext.Provider>
  );
}
