import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, keyAccount, ModelInfo, Preset, ProviderConfig, Settings } from "./api";

type Kind = ProviderConfig["kind"];

const OPENAI_PRESETS: Record<string, { base_url: string; model: string }> = {
  OpenAI: { base_url: "https://api.openai.com/v1", model: "gpt-5-mini" },
  Groq: { base_url: "https://api.groq.com/openai/v1", model: "llama-3.3-70b-versatile" },
  OpenRouter: { base_url: "https://openrouter.ai/api/v1", model: "anthropic/claude-sonnet-5-5" },
  Ollama: { base_url: "http://localhost:11434/v1", model: "llama3.2" },
  "LM Studio": { base_url: "http://localhost:1234/v1", model: "local-model" },
};

function defaultFor(kind: Kind): ProviderConfig {
  switch (kind) {
    case "claude_cli":
      return { kind, model: "sonnet", binary: null };
    case "codex_cli":
      return { kind, model: null, binary: null };
    case "anthropic":
      return { kind, model: "claude-opus-5-5", effort: "low", base_url: null };
    case "open_ai_compatible":
      return { kind, label: "OpenAI", ...OPENAI_PRESETS.OpenAI };
  }
}

export default function SettingsView({ settings, onSaved }: { settings: Settings; onSaved: (s: Settings) => void }) {
  const [draft, setDraft] = useState<Settings>(settings);
  const [clis, setClis] = useState<{ claude: string | null; codex: string | null } | null>(null);
  const [key, setKey] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [saved, setSaved] = useState<string | null>(null);
  const [presetId, setPresetId] = useState(settings.active_preset);

  useEffect(() => setDraft(settings), [settings]);
  useEffect(() => {
    api.detectClis().then(setClis);
    api.listModels().then(setModels);
    const un = listen<{ file: string; downloaded: number; total: number; done: boolean; error: string | null }>("model-download", (e) => {
      const p = e.payload;
      if (p.done) {
        setProgress((x) => {
          const { [p.file]: _, ...rest } = x;
          return rest;
        });
        if (p.error) setSaved(`Unduhan gagal: ${p.error}`);
        api.listModels().then(setModels);
      } else {
        setProgress((x) => ({ ...x, [p.file]: p.total ? p.downloaded / p.total : 0 }));
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const account = keyAccount(draft.provider);
  useEffect(() => {
    setKey("");
    if (account) api.hasApiKey(account).then(setHasKey);
  }, [account]);

  const prov = draft.provider;
  const setProv = (p: ProviderConfig) => setDraft({ ...draft, provider: p });
  const preset = draft.presets.find((p) => p.id === presetId) ?? draft.presets[0];
  const setPreset = (patch: Partial<Preset>) =>
    setDraft({ ...draft, presets: draft.presets.map((p) => (p.id === preset.id ? { ...p, ...patch } : p)) });

  const save = async () => {
    try {
      if (account && key) await api.setApiKey(account, key);
      const s = await api.saveSettings(draft);
      onSaved(s);
      setKey("");
      if (account) setHasKey(await api.hasApiKey(account));
      setSaved("Tersimpan.");
    } catch (e) {
      setSaved(`Gagal menyimpan: ${e}`);
    }
  };

  const addPreset = () => {
    const id = `custom-${Date.now()}`;
    setDraft({
      ...draft,
      presets: [...draft.presets, { id, name: "Preset baru", system_prompt: "Kamu adalah asisten percakapan yang membantu pengguna.", context: "", builtin: false }],
    });
    setPresetId(id);
  };

  return (
    <div className="settings">
      <section className="card">
        <h2>Model AI</h2>
        <div className="radio-grid" role="radiogroup" aria-label="Penyedia AI">
          {(
            [
              ["claude_cli", "Claude (langganan)", clis?.claude ? "CLI ditemukan" : "CLI `claude` belum terpasang"],
              ["codex_cli", "Codex / ChatGPT (langganan)", clis?.codex ? "CLI ditemukan" : "CLI `codex` belum terpasang"],
              ["anthropic", "Anthropic API", "pakai API key"],
              ["open_ai_compatible", "OpenAI-compatible", "OpenAI, Groq, OpenRouter, Ollama, LM Studio"],
            ] as [Kind, string, string][]
          ).map(([k, label, hint]) => (
            <label key={k} className={`radio ${prov.kind === k ? "on" : ""}`}>
              <input type="radio" name="provider" checked={prov.kind === k} onChange={() => setProv(defaultFor(k))} />
              <span className="radio-label">{label}</span>
              <span className="radio-hint">{hint}</span>
            </label>
          ))}
        </div>

        {prov.kind === "claude_cli" && (
          <div className="fields">
            <label className="field">
              <span>Model</span>
              <select value={prov.model} onChange={(e) => setProv({ ...prov, model: e.target.value })}>
                <option value="haiku">Haiku — paling cepat</option>
                <option value="sonnet">Sonnet — seimbang</option>
                <option value="opus">Opus — paling pintar, lebih lambat</option>
              </select>
            </label>
            <p className="hint">
              Memakai login Claude Code kamu sendiri (Pro/Max). BICARA tidak membaca token kamu. Belum login? Jalankan{" "}
              <code>claude</code> sekali di terminal. {clis?.claude && <>Ditemukan: <code>{clis.claude}</code></>}
            </p>
          </div>
        )}
        {prov.kind === "codex_cli" && (
          <div className="fields">
            <label className="field">
              <span>Model (kosongkan untuk default)</span>
              <input value={prov.model ?? ""} onChange={(e) => setProv({ ...prov, model: e.target.value || null })} placeholder="default" />
            </label>
            <p className="hint">
              Memakai login ChatGPT di Codex CLI (gratis atau berbayar). Pasang Codex CLI lalu jalankan <code>codex login</code>.
              Codex mengirim jawaban sekaligus (tidak per kata).
            </p>
          </div>
        )}
        {prov.kind === "anthropic" && (
          <div className="fields">
            <label className="field">
              <span>Model</span>
              <input value={prov.model} onChange={(e) => setProv({ ...prov, model: e.target.value })} />
            </label>
            <label className="field">
              <span>Effort</span>
              <select value={prov.effort} onChange={(e) => setProv({ ...prov, effort: e.target.value })}>
                <option value="low">Low — tercepat</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
              </select>
            </label>
          </div>
        )}
        {prov.kind === "open_ai_compatible" && (
          <div className="fields">
            <label className="field">
              <span>Layanan</span>
              <select
                value={prov.label in OPENAI_PRESETS ? prov.label : "OpenAI"}
                onChange={(e) => setProv({ kind: "open_ai_compatible", label: e.target.value, ...OPENAI_PRESETS[e.target.value] })}
              >
                {Object.keys(OPENAI_PRESETS).map((k) => (
                  <option key={k}>{k}</option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Base URL</span>
              <input value={prov.base_url} onChange={(e) => setProv({ ...prov, base_url: e.target.value })} />
            </label>
            <label className="field">
              <span>Model</span>
              <input value={prov.model} onChange={(e) => setProv({ ...prov, model: e.target.value })} />
            </label>
          </div>
        )}
        {account && (
          <label className="field">
            <span>API key {hasKey ? "(tersimpan di keychain — isi untuk mengganti)" : ""}</span>
            <input type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder={hasKey ? "••••••••" : "tempel API key"} autoComplete="off" />
          </label>
        )}
      </section>

      <section className="card">
        <h2>Transkripsi (offline, whisper)</h2>
        <ul className="models">
          {models.map((m) => (
            <li key={m.file}>
              <label className="check">
                <input
                  type="radio"
                  name="stt"
                  checked={draft.stt_model === m.file}
                  disabled={!m.installed}
                  onChange={() => setDraft({ ...draft, stt_model: m.file })}
                />
                {m.label} <span className="hint">· {m.size_mb} MB</span>
              </label>
              {m.installed ? (
                <span className="status ok">Terpasang</span>
              ) : progress[m.file] !== undefined ? (
                <span className="status">{Math.round(progress[m.file] * 100)}%</span>
              ) : (
                <button className="ghost" onClick={() => api.downloadModel(m.file).catch((e) => setSaved(String(e)))}>
                  Unduh
                </button>
              )}
            </li>
          ))}
        </ul>
        <label className="field">
          <span>Kosakata khusus (nama orang, produk, istilah) — membantu akurasi</span>
          <textarea rows={2} value={draft.vocabulary} onChange={(e) => setDraft({ ...draft, vocabulary: e.target.value })} placeholder="contoh: Tokopedia, OKR, Pak Budi, sprint review" />
        </label>
      </section>

      <section className="card">
        <div className="card-head">
          <h2>Preset & system prompt</h2>
          <button className="ghost" onClick={addPreset}>
            Tambah preset
          </button>
        </div>
        <div className="fields">
          <label className="field">
            <span>Preset</span>
            <select value={preset.id} onChange={(e) => setPresetId(e.target.value)}>
              {draft.presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                  {draft.active_preset === p.id ? " (aktif)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>Nama</span>
            <input value={preset.name} onChange={(e) => setPreset({ name: e.target.value })} />
          </label>
        </div>
        <label className="field">
          <span>System prompt (peran & aturan AI)</span>
          <textarea rows={5} value={preset.system_prompt} onChange={(e) => setPreset({ system_prompt: e.target.value })} />
        </label>
        <label className="field">
          <span>Konteks (tempel CV, deskripsi pekerjaan, agenda, catatan produk…)</span>
          <textarea rows={6} value={preset.context} onChange={(e) => setPreset({ context: e.target.value })} placeholder="Teks ini dikirim ke AI bersama transkrip." />
        </label>
        <div className="row">
          <button className="ghost" onClick={() => setDraft({ ...draft, active_preset: preset.id })} disabled={draft.active_preset === preset.id}>
            Jadikan aktif
          </button>
          {preset.builtin ? (
            <button
              className="ghost"
              onClick={async () => {
                const p = await api.resetPreset(preset.id);
                if (p) setPreset(p);
              }}
            >
              Kembalikan prompt bawaan
            </button>
          ) : (
            <button
              className="ghost"
              onClick={() => {
                const presets = draft.presets.filter((p) => p.id !== preset.id);
                setDraft({ ...draft, presets, active_preset: draft.active_preset === preset.id ? presets[0].id : draft.active_preset });
                setPresetId(presets[0].id);
              }}
            >
              Hapus preset
            </button>
          )}
        </div>
      </section>

      <div className="savebar">
        {saved && <span role="status">{saved}</span>}
        <button className="primary" onClick={save}>
          Simpan pengaturan
        </button>
      </div>
    </div>
  );
}
