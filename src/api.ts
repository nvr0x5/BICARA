import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export type AudioApp = {
  id: string;
  name: string;
  pids: number[];
  playing: boolean;
  is_app: boolean;
  icon_png_base64: string | null;
};

export type Level = { source: "app" | "mic"; rms: number; peak: number };

export const api = {
  listAudioApps: () => invoke<AudioApp[]>("list_audio_apps"),
  startCapture: (appId: string | null, mic: boolean) =>
    invoke<{ dir: string }>("start_capture", { appId, mic }),
  stopCapture: () => invoke<string[]>("stop_capture"),
  recordingsDir: () => invoke<string>("recordings_dir"),
  setStealth: (enabled: boolean) => invoke<boolean>("set_stealth", { enabled }),
  getStealth: () => invoke<boolean>("get_stealth"),
  toggleVisibility: () => invoke<void>("toggle_visibility"),
  panicHotkeyLabel: () => invoke<string>("panic_hotkey_label"),
};

/** Live audio levels per source, decaying to zero when no events arrive. */
export function useLevels() {
  const [levels, setLevels] = useState<Record<string, Level>>({});
  useEffect(() => {
    const un = listen<Level>("audio-level", (e) =>
      setLevels((l) => ({ ...l, [e.payload.source]: e.payload })),
    );
    return () => {
      un.then((f) => f());
    };
  }, []);
  return levels;
}

/** Map RMS to a 0..1 meter value on a dB scale (-60 dB .. 0 dB). */
export function meter(rms: number | undefined) {
  if (!rms || rms <= 0) return 0;
  const db = 20 * Math.log10(rms);
  return Math.max(0, Math.min(1, (db + 60) / 60));
}
