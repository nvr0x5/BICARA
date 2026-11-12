import { useEffect, useState } from "react";
import { api, Settings } from "./api";
import Live from "./Live";
import SettingsView from "./SettingsView";
import StealthView from "./StealthView";

type Tab = "live" | "settings" | "stealth";
const TABS: [Tab, string][] = [
  ["live", "Live"],
  ["settings", "Pengaturan"],
  ["stealth", "Siluman"],
];

export default function App() {
  const [tab, setTab] = useState<Tab>("live");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [hotkeys, setHotkeys] = useState({ panic: "", assist: "" });

  useEffect(() => {
    api.getSettings().then(setSettings);
    api.hotkeys().then(setHotkeys);
  }, []);

  return (
    <main className="app">
      <header className="top">
        <h1>BICARA</h1>
        <nav className="tabs" role="tablist">
          {TABS.map(([id, label]) => (
            <button key={id} role="tab" aria-selected={tab === id} className={tab === id ? "on" : ""} onClick={() => setTab(id)}>
              {label}
            </button>
          ))}
        </nav>
      </header>
      {/* Live stays mounted so recording state and streams survive tab switches. */}
      <div hidden={tab !== "live"}>
        <Live settings={settings} onSettings={setSettings} assistKey={hotkeys.assist} />
      </div>
      {tab === "settings" && settings && <SettingsView settings={settings} onSaved={setSettings} />}
      {tab === "stealth" && <StealthView hotkeys={hotkeys} />}
    </main>
  );
}
