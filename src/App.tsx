import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { LangContext, translate } from "./i18n";
import { api, applyTheme, isMac, Meeting as M, Settings } from "./api";
import Meeting from "./Meeting";
import History from "./History";
import SettingsWindow from "./SettingsWindow";
import { IClock, IDoc, IEyeOff, IGear, IPlus, ISidebar, IWave } from "./Icons";

export type View = { kind: "meeting" } | { kind: "history"; id: number | null } | { kind: "settings" };

/** Groups meetings like Notes/Mail: Hari ini, Kemarin, 7 hari terakhir, Lebih lama. */
function groupOf(ts: number): string {
  const d = new Date(ts * 1000);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const diff = (today.getTime() - new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()) / 86400000;
  if (diff <= 0) return "Hari ini";
  if (diff === 1) return "Kemarin";
  if (diff < 7) return "7 hari terakhir";
  return "Lebih lama";
}

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [hotkeys, setHotkeys] = useState({ panic: "", assist: "" });
  const [view, setView] = useState<View>({ kind: "meeting" });
  const [meetings, setMeetings] = useState<M[]>([]);
  const [live, setLive] = useState<M | null>(null);
  const [stealth, setStealth] = useState(false);
  const [sidebar, setSidebar] = useState(true);

  const reload = useCallback(() => api.listMeetings().then(setMeetings).catch(() => {}), []);

  useEffect(() => {
    api.getSettings().then(setSettings);
    api.hotkeys().then(setHotkeys);
    api.getStealth().then(setStealth).catch(() => {});
    reload();
    const uns = [
      listen("meeting-state", reload),
      listen("notulen-ready", reload),
      listen<Settings>("settings-changed", (e) => setSettings(e.payload)),
      listen<string>("menu", (e) => {
        if (e.payload === "new") setView({ kind: "meeting" });
        if (e.payload === "history") setView({ kind: "history", id: null });
        if (e.payload === "toggle-sidebar") setSidebar((s) => !s);
      }),
    ];
    return () => uns.forEach((u) => u.then((f) => f()));
  }, [reload]);

  useEffect(() => applyTheme(settings?.theme), [settings?.theme]);

  // Ctrl+, opens settings on Windows (macOS gets ⌘, from the menu bar).
  useEffect(() => {
    if (isMac) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === ",") setView({ kind: "settings" });
      if (e.ctrlKey && e.key.toLowerCase() === "n") setView({ kind: "meeting" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const lang = settings?.ui_language ?? "id";
  const tr = (s: string) => translate(lang, s);
  const openSettings = () => (isMac ? api.openSettings() : setView({ kind: "settings" }));
  const toggleStealth = async () => setStealth(await api.setStealth(!stealth));

  const past = meetings.filter((m) => m.id !== live?.id).slice(0, 12);
  const groups: [string, M[]][] = [];
  for (const m of past) {
    const g = groupOf(m.started_at);
    const last = groups[groups.length - 1];
    if (last && last[0] === g) last[1].push(m);
    else groups.push([g, [m]]);
  }
  const isOn = (v: View) => JSON.stringify(v) === JSON.stringify(view);

  return (
    <LangContext.Provider value={lang}>
      <main className={`shell ${sidebar ? "" : "no-sidebar"}`} lang={lang}>
        {sidebar && (
          <aside className="sidebar">
            <div className="drag" data-tauri-drag-region />
            <nav className="src" aria-label={tr("Navigasi")}>
              <button className={isOn({ kind: "meeting" }) && !live ? "on" : ""} onClick={() => setView({ kind: "meeting" })}>
                <IPlus />
                <span className="t">{tr("Meeting baru")}</span>
              </button>
              {live && (
                <>
                  <h6>{tr("Sekarang")}</h6>
                  <button className={isOn({ kind: "meeting" }) ? "on" : ""} onClick={() => setView({ kind: "meeting" })}>
                    <IWave />
                    <span className="t">{live.title}</span>
                    <i className="live-dot" aria-label={tr("Merekam")} />
                  </button>
                </>
              )}
              {groups.map(([g, ms]) => (
                <div key={g}>
                  <h6>{tr(g)}</h6>
                  {ms.map((m) => (
                    <button key={m.id} className={isOn({ kind: "history", id: m.id }) ? "on" : ""} onClick={() => setView({ kind: "history", id: m.id })} title={m.title}>
                      <IDoc />
                      <span className="t">{m.title}</span>
                    </button>
                  ))}
                </div>
              ))}
              <h6>{tr("Riwayat")}</h6>
              <button className={isOn({ kind: "history", id: null }) ? "on" : ""} onClick={() => setView({ kind: "history", id: null })}>
                <IClock />
                <span className="t">{tr("Semua meeting")}</span>
                <span className="count">{meetings.length}</span>
              </button>
            </nav>
            <div className="side-foot">
              <button className={`tb ${stealth ? "on" : ""}`} onClick={toggleStealth} aria-pressed={stealth} title={tr("Sembunyikan BICARA dari share layar")}>
                <IEyeOff />
                {stealth ? tr("Siluman aktif") : tr("Siluman mati")}
              </button>
              <span className="spacer" />
              <button className={`tb ${view.kind === "settings" ? "on" : ""}`} onClick={openSettings} title={tr("Pengaturan")} aria-label={tr("Pengaturan")}>
                <IGear />
              </button>
            </div>
          </aside>
        )}
        <section className="pane">
          {/* The meeting view stays mounted so recording state and streams survive navigation. */}
          <div className="view" hidden={view.kind !== "meeting"}>
            <Meeting
              settings={settings}
              onSettings={setSettings}
              assistKey={hotkeys.assist}
              onLive={setLive}
              onOpenHistory={(id) => setView({ kind: "history", id })}
              sidebarToggle={
                <button className="tb" onClick={() => setSidebar((s) => !s)} aria-label={tr("Tampilkan/sembunyikan sidebar")} aria-pressed={!sidebar}>
                  <ISidebar />
                </button>
              }
            />
          </div>
          {view.kind === "history" && (
            <div className="view">
              <History openId={view.id} onDeleted={reload} />
            </div>
          )}
          {view.kind === "settings" && !isMac && settings && (
            <div className="view">
              <SettingsWindow embedded />
            </div>
          )}
        </section>
      </main>
    </LangContext.Provider>
  );
}
