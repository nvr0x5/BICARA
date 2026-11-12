import { useEffect, useState } from "react";
import { useT, presetName, UiLang } from "./i18n";
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
  const t = useT();
  const [draft, setDraft] = useState<Settings>(settings);
  const [clis, setClis] = useState<{ claude: string | null; codex: string | null } | null>(null);
  const [key, setKey] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [saved, setSaved] = useState<string | null>(null);
  const [presetId, setPresetId] = useState(settings.active_preset);
  const [mics, setMics] = useState<string[]>([]);

  useEffect(() => setDraft(settings), [settings]);
  useEffect(() => {
    api.detectClis().then(setClis);
    api.listModels().then(setModels);
    api.listMicrophones().then(setMics);
    const un = listen<{ file: string; downloaded: number; total: number; done: boolean; error: string | null }>("model-download", (e) => {
      const p = e.payload;
      if (p.done) {
        setProgress((x) => {
          const { [p.file]: _, ...rest } = x;
          return rest;
        });
        if (p.error) setSaved(t("Unduhan gagal: {error}", { error: p.error }));
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
      setSaved(t("Gagal menyimpan: {error}", { error: String(e) }));
    }
  };

  const addPreset = () => {
    const id = `custom-${Date.now()}`;
    setDraft({
      ...draft,
      presets: [...draft.presets, { id, name: t("Preset baru"), system_prompt: t("Kamu adalah asisten percakapan yang membantu pengguna."), context: "", builtin: false }],
    });
    setPresetId(id);
  };

  return (
    <div className="settings">
      <section className="card">
        <h2>{t("Model AI")}</h2>
        <div className="radio-grid" role="radiogroup" aria-label={t("Penyedia AI")}>
          {(
            [
              ["claude_cli", t("Claude (langganan)"), clis?.claude ? t("CLI ditemukan") : t("CLI `claude` belum terpasang")],
              ["codex_cli", t("Codex / ChatGPT (langganan)"), clis?.codex ? t("CLI ditemukan") : t("CLI `codex` belum terpasang")],
              ["anthropic", t("Anthropic API"), t("pakai API key")],
              ["open_ai_compatible", t("OpenAI-compatible"), "OpenAI, Groq, OpenRouter, Ollama, LM Studio"],
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
              <span>{t("Model")}</span>
              <select value={prov.model} onChange={(e) => setProv({ ...prov, model: e.target.value })}>
                <option value="haiku">{t("Haiku — paling cepat")}</option>
                <option value="sonnet">{t("Sonnet — seimbang")}</option>
                <option value="opus">{t("Opus — paling pintar, lebih lambat")}</option>
              </select>
            </label>
            <p className="hint">
              {t("Memakai login Claude Code kamu sendiri (Pro/Max). BICARA tidak membaca token kamu. Belum login? Jalankan")}{" "}
              <code>claude</code> {t("sekali di terminal.")} {clis?.claude && <>{t("Ditemukan:")} <code>{clis.claude}</code></>}
            </p>
          </div>
        )}
        {prov.kind === "codex_cli" && (
          <div className="fields">
            <label className="field">
              <span>{t("Model (kosongkan untuk default)")}</span>
              <input value={prov.model ?? ""} onChange={(e) => setProv({ ...prov, model: e.target.value || null })} placeholder="default" />
            </label>
            <p className="hint">
              {t("Memakai login ChatGPT di Codex CLI (gratis atau berbayar). Pasang Codex CLI lalu jalankan")} <code>codex login</code>.
              {t("Codex mengirim jawaban sekaligus (tidak per kata).")}
            </p>
          </div>
        )}
        {prov.kind === "anthropic" && (
          <div className="fields">
            <label className="field">
              <span>{t("Model")}</span>
              <input value={prov.model} onChange={(e) => setProv({ ...prov, model: e.target.value })} />
            </label>
            <label className="field">
              <span>{t("Effort")}</span>
              <select value={prov.effort} onChange={(e) => setProv({ ...prov, effort: e.target.value })}>
                <option value="low">{t("Low — tercepat")}</option>
                <option value="medium">{t("Medium")}</option>
                <option value="high">{t("High")}</option>
              </select>
            </label>
          </div>
        )}
        {prov.kind === "open_ai_compatible" && (
          <div className="fields">
            <label className="field">
              <span>{t("Layanan")}</span>
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
              <span>{t("Base URL")}</span>
              <input value={prov.base_url} onChange={(e) => setProv({ ...prov, base_url: e.target.value })} />
            </label>
            <label className="field">
              <span>{t("Model")}</span>
              <input value={prov.model} onChange={(e) => setProv({ ...prov, model: e.target.value })} />
            </label>
          </div>
        )}
        {account && (
          <label className="field">
            <span>{t("API key")} {hasKey ? t("(tersimpan di keychain — isi untuk mengganti)") : ""}</span>
            <input type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder={hasKey ? "••••••••" : t("tempel API key")} autoComplete="off" />
          </label>
        )}
      </section>

      <section className="card">
        <h2>{t("Bahasa & mikrofon")}</h2>
        <div className="fields">
          <label className="field">
            <span>{t("Bahasa aplikasi")} / App language</span>
            <select value={draft.ui_language ?? "id"} onChange={(e) => setDraft({ ...draft, ui_language: e.target.value as UiLang })}>
              <option value="id">Bahasa Indonesia</option>
              <option value="en">English</option>
            </select>
          </label>
          <label className="field">
            <span>{t("Bahasa percakapan (transkripsi & jawaban AI)")}</span>
            <select value={draft.language} onChange={(e) => setDraft({ ...draft, language: e.target.value as Settings["language"] })}>
              <option value="id">{t("Bahasa Indonesia")}</option>
              <option value="en">{t("English")}</option>
              <option value="auto">{t("Otomatis (Indonesia / English)")}</option>
            </select>
          </label>
          <label className="field">
            <span>{t("Mikrofon")}</span>
            <select value={draft.mic_device ?? ""} onChange={(e) => setDraft({ ...draft, mic_device: e.target.value || null })}>
              <option value="">{t("Default sistem")}</option>
              {mics.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </label>
        </div>
        <p className="hint">{t("“Otomatis” cocok untuk meeting campuran, tapi sedikit kurang akurat dibanding memilih satu bahasa.")}</p>
      </section>

      <section className="card">
        <h2>{t("Transkripsi (offline, whisper)")}</h2>
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
                {t(m.label)} <span className="hint">· {m.size_mb} MB</span>
              </label>
              {m.installed ? (
                <span className="status ok">{t("Terpasang")}</span>
              ) : progress[m.file] !== undefined ? (
                <span className="status">{Math.round(progress[m.file] * 100)}%</span>
              ) : (
                <button className="ghost" onClick={() => api.downloadModel(m.file).catch((e) => setSaved(String(e)))}>
                  {t("Unduh")}
                </button>
              )}
            </li>
          ))}
        </ul>
        <label className="field">
          <span>{t("Kosakata khusus (nama orang, produk, istilah) — membantu akurasi")}</span>
          <textarea rows={2} value={draft.vocabulary} onChange={(e) => setDraft({ ...draft, vocabulary: e.target.value })} placeholder={t("contoh: Tokopedia, OKR, Pak Budi, sprint review")} />
        </label>
      </section>

      <section className="card">
        <div className="card-head">
          <h2>{t("Preset & system prompt")}</h2>
          <button className="ghost" onClick={addPreset}>
            {t("Tambah preset")}
          </button>
        </div>
        <div className="fields">
          <label className="field">
            <span>{t("Preset")}</span>
            <select value={preset.id} onChange={(e) => setPresetId(e.target.value)}>
              {draft.presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {presetName(t, p)}
                  {draft.active_preset === p.id ? ` ${t("(aktif)")}` : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>{t("Nama")}</span>
            <input value={preset.name} onChange={(e) => setPreset({ name: e.target.value })} />
          </label>
        </div>
        <label className="field">
          <span>{t("System prompt (peran & aturan AI)")}</span>
          <textarea rows={5} value={preset.system_prompt} onChange={(e) => setPreset({ system_prompt: e.target.value })} />
        </label>
        <label className="field">
          <span>{t("Konteks (tempel CV, deskripsi pekerjaan, agenda, catatan produk…)")}</span>
          <textarea rows={6} value={preset.context} onChange={(e) => setPreset({ context: e.target.value })} placeholder={t("Teks ini dikirim ke AI bersama transkrip.")} />
        </label>
        <div className="row">
          <button className="ghost" onClick={() => setDraft({ ...draft, active_preset: preset.id })} disabled={draft.active_preset === preset.id}>
            {t("Jadikan aktif")}
          </button>
          {preset.builtin ? (
            <button
              className="ghost"
              onClick={async () => {
                const p = await api.resetPreset(preset.id);
                if (p) setPreset(p);
              }}
            >
              {t("Kembalikan prompt bawaan")}
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
              {t("Hapus preset")}
            </button>
          )}
        </div>
      </section>

      <section className="card">
        <h2>{t("Privasi")}</h2>
        <label className="check">
          <input type="checkbox" checked={draft.consent_reminder} onChange={(e) => setDraft({ ...draft, consent_reminder: e.target.checked })} />
          {t("Ingatkan saya memberi tahu peserta sebelum merekam (UU PDP)")}
        </label>
        <label className="check">
          <input type="checkbox" checked={draft.keep_audio} onChange={(e) => setDraft({ ...draft, keep_audio: e.target.checked })} />
          {t("Simpan rekaman audio setiap meeting (transkrip & notulen selalu disimpan)")}
        </label>
        <p className="hint">{t("Semua data tersimpan lokal di komputer ini. Teks transkrip dikirim ke penyedia AI yang kamu pilih saat AI dipakai.")}</p>
      </section>

      <div className="savebar">
        {saved && <span role="status">{t(saved)}</span>}
        <button className="primary" onClick={save}>
          {t("Simpan pengaturan")}
        </button>
      </div>
    </div>
  );
}
