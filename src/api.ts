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

export type Utterance = { source: "app" | "mic"; text: string; start: number; end: number; latency_ms: number };

export type ProviderConfig =
  | { kind: "claude_cli"; model: string; binary?: string | null }
  | { kind: "codex_cli"; model?: string | null; binary?: string | null }
  | { kind: "anthropic"; model: string; effort: string; base_url?: string | null }
  | { kind: "open_ai_compatible"; label: string; base_url: string; model: string };

export type Preset = { id: string; name: string; system_prompt: string; context: string; builtin: boolean };

export type Settings = {
  provider: ProviderConfig;
  stt_model: string;
  vocabulary: string;
  register: "formal" | "santai";
  auto_assist: boolean;
  active_preset: string;
  presets: Preset[];
  consent_reminder: boolean;
  keep_audio: boolean;
  language: "id" | "en" | "auto";
  mic_device: string | null;
  ui_language: "id" | "en";
};

export type Meeting = {
  id: number;
  title: string;
  preset_id: string;
  app_name: string | null;
  started_at: number;
  ended_at: number | null;
  notulen: string | null;
  recording_dir: string | null;
  utterances: number;
  documents: number;
};
export type Doc = { id: number; meeting_id: number; name: string; chars: number };
export type StoredAnswer = { id: number; action: string; title: string; input: string | null; text: string; created_at: number };
export type MeetingDetail = { meeting: Meeting; date: string; utterances: Utterance[]; answers: StoredAnswer[]; documents: Doc[] };
export type SearchHit = { meeting_id: number; title: string; started_at: number; snippet: string };

export type ModelInfo = { file: string; label: string; size_mb: number; installed: boolean };
export type Action = "assist" | "ask" | "summary" | "followup" | "notulen" | "prep";

export const api = {
  listAudioApps: () => invoke<AudioApp[]>("list_audio_apps"),
  startCapture: (appId: string | null, mic: boolean) =>
    invoke<{ dir: string; transcribing: boolean; warning: string | null }>("start_capture", { appId, mic }),
  stopCapture: () => invoke<string[]>("stop_capture"),
  setStealth: (enabled: boolean) => invoke<boolean>("set_stealth", { enabled }),
  getStealth: () => invoke<boolean>("get_stealth"),
  hotkeys: () => invoke<{ panic: string; assist: string }>("hotkey_labels"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  resetPreset: (id: string) => invoke<Preset | null>("reset_preset", { id }),
  setApiKey: (account: string, key: string) => invoke<void>("set_api_key", { account, key }),
  hasApiKey: (account: string) => invoke<boolean>("has_api_key", { account }),
  detectClis: () => invoke<{ claude: string | null; codex: string | null }>("detect_clis"),
  listMicrophones: () => invoke<string[]>("list_microphones"),
  listModels: () => invoke<ModelInfo[]>("list_models"),
  downloadModel: (file: string) => invoke<void>("download_model", { file }),
  sttLoaded: () => invoke<boolean>("stt_loaded"),
  aiAction: (action: Action, input?: string) => invoke<number>("ai_action", { action, input: input ?? null }),
  getTranscript: () => invoke<Utterance[]>("get_transcript"),
  clearTranscript: () => invoke<void>("clear_transcript"),
  createMeeting: (title: string, presetId: string) => invoke<Meeting>("create_meeting", { title, presetId }),
  addMeetingFiles: (meetingId: number, paths: string[]) =>
    invoke<{ name: string; document: Doc | null; error: string | null }[]>("add_meeting_files", { meetingId, paths }),
  removeMeetingDocument: (documentId: number) => invoke<void>("remove_meeting_document", { documentId }),
  docExtensions: () => invoke<string[]>("supported_document_extensions"),
  startMeeting: (meetingId: number, appId: string | null, appName: string | null, mic: boolean) =>
    invoke<{ meeting: Meeting; transcribing: boolean; warning: string | null }>("start_meeting", { meetingId, appId, appName, mic }),
  endMeeting: () => invoke<number | null>("end_meeting"),
  currentMeeting: () => invoke<Meeting | null>("current_meeting"),
  generateNotulen: (meetingId: number) => invoke<number>("generate_notulen", { meetingId }),
  saveNotulen: (meetingId: number, text: string) => invoke<void>("save_notulen", { meetingId, text }),
  prepareInterview: (meetingId: number) => invoke<number>("prepare_interview", { meetingId }),
  listMeetings: () => invoke<Meeting[]>("list_meetings"),
  searchMeetings: (query: string) => invoke<SearchHit[]>("search_meetings", { query }),
  getMeeting: (meetingId: number) => invoke<MeetingDetail>("get_meeting", { meetingId }),
  renameMeeting: (meetingId: number, title: string) => invoke<void>("rename_meeting", { meetingId, title }),
  deleteMeeting: (meetingId: number) => invoke<void>("delete_meeting", { meetingId }),
  askMeeting: (meetingId: number, question: string) => invoke<number>("ask_meeting", { meetingId, question }),
  exportMeeting: (meetingId: number, path: string) => invoke<string>("export_meeting", { meetingId, path }),
  whatsappUrl: (meetingId: number) => invoke<string>("whatsapp_share_url", { meetingId }),
};

export function fmtDate(ts: number) {
  const lang = document.querySelector("main")?.getAttribute("lang") === "en" ? "en-GB" : "id-ID";
  return new Date(ts * 1000).toLocaleString(lang, { dateStyle: "medium", timeStyle: "short" });
}

export function keyAccount(p: ProviderConfig): string | null {
  if (p.kind === "anthropic") return "anthropic";
  if (p.kind === "open_ai_compatible") return `openai-compatible:${p.label.toLowerCase()}`;
  return null;
}

/** Live audio levels per source. */
export function useLevels() {
  const [levels, setLevels] = useState<Record<string, Level>>({});
  useEffect(() => {
    const un = listen<Level>("audio-level", (e) => setLevels((l) => ({ ...l, [e.payload.source]: e.payload })));
    return () => {
      un.then((f) => f());
    };
  }, []);
  return levels;
}

/** Live transcript (initial fetch + streaming updates). */
export function useTranscript() {
  const [lines, setLines] = useState<Utterance[]>([]);
  useEffect(() => {
    api.getTranscript().then(setLines);
    const un = listen<Utterance>("transcript", (e) => setLines((l) => [...l, e.payload]));
    return () => {
      un.then((f) => f());
    };
  }, []);
  return [lines, setLines] as const;
}

export type AiAnswer = {
  id: number;
  action: Action;
  title: string;
  provider: string;
  text: string;
  done: boolean;
  error: string | null;
  ms: number | null;
};

/** Streaming AI answers, newest first. */
export function useAnswers() {
  const [answers, setAnswers] = useState<AiAnswer[]>([]);
  useEffect(() => {
    const uns = [
      listen<{ id: number; action: Action; title: string; provider: string }>("ai-start", (e) =>
        setAnswers((a) => [{ ...e.payload, text: "", done: false, error: null, ms: null }, ...a].slice(0, 30)),
      ),
      listen<{ id: number; delta: string }>("ai-delta", (e) =>
        setAnswers((a) => a.map((x) => (x.id === e.payload.id ? { ...x, text: x.text + e.payload.delta } : x))),
      ),
      listen<{ id: number; text: string; error: string | null; ms: number }>("ai-done", (e) =>
        setAnswers((a) =>
          a.map((x) =>
            x.id === e.payload.id
              ? { ...x, text: e.payload.text || x.text, error: e.payload.error, done: true, ms: e.payload.ms }
              : x,
          ),
        ),
      ),
    ];
    return () => {
      uns.forEach((u) => u.then((f) => f()));
    };
  }, []);
  return answers;
}

export function useLastQuestion() {
  const [q, setQ] = useState<string | null>(null);
  useEffect(() => {
    const uns = [
      listen<string>("question-detected", (e) => setQ(e.payload)),
      // A meeting started or ended: the previous question no longer applies.
      listen("meeting-state", () => setQ(null)),
    ];
    return () => {
      uns.forEach((u) => u.then((f) => f()));
    };
  }, []);
  return q;
}

/** Map RMS to a 0..1 meter value on a dB scale (-60 dB .. 0 dB). */
export function meter(rms: number | undefined) {
  if (!rms || rms <= 0) return 0;
  const db = 20 * Math.log10(rms);
  return Math.max(0, Math.min(1, (db + 60) / 60));
}

export function fmtTime(t: number) {
  const s = Math.floor(t);
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}
