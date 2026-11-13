import { useEffect, useState } from "react";
import { useT, presetName, UiLang } from "./i18n";
import { listen } from "@tauri-apps/api/event";
import { api, keyAccount, ModelInfo, Preset, ProviderConfig, Settings } from "./api";
import { IRefresh } from "./Icons";

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

export type SettingsTab = "umum" | "ai" | "transkripsi" | "preset" | "siluman" | "privasi";

export default function SettingsView({ settings, onSaved, tab }: { settings: Settings; onSaved: (s: Settings) => void; tab: SettingsTab }) {
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
    <>
    <div className="scroll"><div className="settings">
      {tab === "ai" && (
        <AiTab
          prov={prov}
          setProv={setProv}
          clis={clis}
          account={account}
          apiKey={key}
          setApiKey={setKey}
          hasKey={hasKey}
        />
      )}

      {tab === "umum" && (
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
            <span>{t("Tema")}</span>
            <select value={draft.theme ?? "system"} onChange={(e) => setDraft({ ...draft, theme: e.target.value as Settings["theme"] })}>
              <option value="system">{t("Ikuti sistem")}</option>
              <option value="dark">{t("Gelap")}</option>
              <option value="light">{t("Terang")}</option>
            </select>
          </label>
          <label className="field">
            <span>{t("Terjemahan langsung")}</span>
            <select value={draft.translate_to ?? ""} onChange={(e) => setDraft({ ...draft, translate_to: e.target.value as Settings["translate_to"] })}>
              <option value="">{t("Mati")}</option>
              <option value="en">{t("Ke English")}</option>
              <option value="id">{t("Ke Bahasa Indonesia")}</option>
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
        <label className="check">
          <input type="checkbox" checked={draft.live_partials ?? true} onChange={(e) => setDraft({ ...draft, live_partials: e.target.checked })} />
          {t("Tampilkan teks sementara saat orang masih bicara")}
        </label>
        <p className="hint">{t("Terjemahan memakai penyedia AI (Haiku untuk langganan Claude), satu baris setiap kali.")}</p>
        <p className="hint">{t("“Otomatis” cocok untuk meeting campuran, tapi sedikit kurang akurat dibanding memilih satu bahasa.")}</p>
      </section>
      )}

      {tab === "transkripsi" && (
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
      )}

      {tab === "preset" && (
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
      )}

      {tab === "privasi" && (
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
        <div className="row between">
          <span className="hint">{t("Log aplikasi membantu saat melaporkan masalah. Isinya langkah teknis, bukan transkrip.")}</span>
          <button className="push" onClick={() => api.openLogDir().catch((e) => setSaved(String(e)))}>
            {t("Buka folder log")}
          </button>
        </div>
      </section>
      )}

    </div></div>
      <div className="savebar">
        {saved && <span role="status">{t(saved)}</span>}
        <button className="primary" onClick={save}>
          {t("Simpan pengaturan")}
        </button>
      </div>
    </>
  );
}

// ------------------------------------------------------------------ AI tab

const KIND_LABEL: Record<Kind, string> = {
  claude_cli: "Claude (langganan)",
  codex_cli: "Codex / ChatGPT (langganan)",
  anthropic: "Anthropic API",
  open_ai_compatible: "OpenAI-compatible",
};

type TestState = { busy: boolean; ok?: boolean; text?: string };

function AiTab({
  prov,
  setProv,
  clis,
  account,
  apiKey,
  setApiKey,
  hasKey,
}: {
  prov: ProviderConfig;
  setProv: (p: ProviderConfig) => void;
  clis: { claude: string | null; codex: string | null } | null;
  account: string | null;
  apiKey: string;
  setApiKey: (k: string) => void;
  hasKey: boolean;
}) {
  const t = useT();
  const [models, setModels] = useState<{ id: string; label: string }[]>([]);
  const [modelsErr, setModelsErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [test, setTest] = useState<TestState>({ busy: false });

  const canList = prov.kind === "claude_cli" || prov.kind === "open_ai_compatible" || (prov.kind === "anthropic" && (hasKey || !!apiKey));
  const fetchModels = async () => {
    if (prov.kind === "codex_cli" || !canList) {
      setModels([]);
      return;
    }
    setLoading(true);
    setModelsErr(null);
    try {
      setModels(await api.listProviderModels(prov, apiKey || null));
    } catch (e) {
      setModels([]);
      setModelsErr(String(e));
    }
    setLoading(false);
  };
  const listKey = `${prov.kind}|${"base_url" in prov ? prov.base_url : ""}|${hasKey}`;
  useEffect(() => {
    setTest({ busy: false });
    fetchModels();
  }, [listKey]);

  const runTest = async () => {
    setTest({ busy: true });
    try {
      const r = await api.testProvider(prov, apiKey || null);
      setTest({ busy: false, ok: true, text: t("Terhubung · {model} · {s} dtk", { model: r.model ?? t("model default"), s: (r.ms / 1000).toFixed(1) }) });
    } catch (e) {
      setTest({ busy: false, ok: false, text: String(e) });
    }
  };

  const cliPath = prov.kind === "claude_cli" ? clis?.claude : prov.kind === "codex_cli" ? clis?.codex : null;
  const modelValue = "model" in prov ? (prov.model ?? "") : "";
  const setModel = (m: string) => setProv({ ...prov, model: prov.kind === "codex_cli" ? m || null : m } as ProviderConfig);
  const modelPicker =
    models.length > 0 ? (
      <select value={modelValue} onChange={(e) => setModel(e.target.value)}>
        {!models.some((m) => m.id === modelValue) && modelValue && <option value={modelValue}>{modelValue}</option>}
        {models.map((m) => (
          <option key={m.id} value={m.id}>
            {m.label === m.id || prov.kind === "claude_cli" ? m.label : `${m.label} — ${m.id}`}
          </option>
        ))}
      </select>
    ) : (
      <input value={modelValue} onChange={(e) => setModel(e.target.value)} placeholder={prov.kind === "codex_cli" ? t("default") : t("id model")} />
    );

  return (
    <>
      <div className="group">
        <h4>{t("Penyedia")}</h4>
        <div className="box">
          <label className="item">
            <span className="lbl">{t("Penyedia AI")}</span>
            <select value={prov.kind} onChange={(e) => setProv(defaultFor(e.target.value as Kind))}>
              {(Object.keys(KIND_LABEL) as Kind[]).map((k) => (
                <option key={k} value={k}>
                  {t(KIND_LABEL[k])}
                </option>
              ))}
            </select>
          </label>
          {(prov.kind === "claude_cli" || prov.kind === "codex_cli") && (
            <div className="item">
              <span className="lbl">
                {t("CLI")}
                <small className="selectable">{cliPath ?? t("Belum terpasang")}</small>
              </span>
              <span className={`status ${cliPath ? "ok" : ""}`}>{cliPath ? t("Ditemukan") : t("Tidak ditemukan")}</span>
            </div>
          )}
        </div>
        <p className="hint">
          {prov.kind === "claude_cli" && t("Memakai login Claude Code kamu sendiri (Pro/Max). BICARA tidak membaca token kamu. Belum login? Jalankan `claude` sekali di terminal.")}
          {prov.kind === "codex_cli" && t("Memakai login ChatGPT di Codex CLI (gratis atau berbayar). Jalankan `codex login` sekali. Jawaban datang sekaligus, tidak per kata.")}
          {prov.kind === "anthropic" && t("Bayar per pemakaian dengan API key dari console.anthropic.com. Key disimpan di keychain.")}
          {prov.kind === "open_ai_compatible" && t("OpenAI, Groq, OpenRouter, atau model lokal lewat Ollama / LM Studio.")}
        </p>
      </div>

      <div className="group">
        <h4>{t("Koneksi")}</h4>
        <div className="box">
          {prov.kind === "open_ai_compatible" && (
            <>
              <label className="item">
                <span className="lbl">{t("Layanan")}</span>
                <select
                  value={prov.label in OPENAI_PRESETS ? prov.label : "OpenAI"}
                  onChange={(e) => setProv({ kind: "open_ai_compatible", label: e.target.value, ...OPENAI_PRESETS[e.target.value] })}
                >
                  {Object.keys(OPENAI_PRESETS).map((k) => (
                    <option key={k}>{k}</option>
                  ))}
                </select>
              </label>
              <label className="item">
                <span className="lbl">{t("Base URL")}</span>
                <input value={prov.base_url} onChange={(e) => setProv({ ...prov, base_url: e.target.value })} />
              </label>
            </>
          )}
          {account && (
            <label className="item">
              <span className="lbl">
                {t("API key")}
                <small>{hasKey ? t("Tersimpan di keychain — isi untuk mengganti") : t("Belum ada")}</small>
              </span>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                onBlur={fetchModels}
                placeholder={hasKey ? "••••••••" : t("tempel API key")}
                autoComplete="off"
              />
            </label>
          )}
          <div className="item">
            <label className="lbl" htmlFor="ai-model">
              {t("Model")}
              <small>
                {prov.kind === "claude_cli"
                  ? t("“Terbaru” ikut versi baru otomatis; versi bernomor tetap")
                  : prov.kind === "codex_cli"
                    ? t("Kosongkan untuk default Codex")
                    : loading
                      ? t("Mengambil daftar model…")
                      : modelsErr
                        ? t("Daftar model tidak bisa diambil — ketik manual")
                        : models.length
                          ? t("{n} model tersedia", { n: String(models.length) })
                          : t("Isi API key untuk mengambil daftar model")}
              </small>
            </label>
            <span className="row" style={{ flexWrap: "nowrap", width: "60%", justifyContent: "flex-end" }} id="ai-model">
              {modelPicker}
              {prov.kind !== "claude_cli" && prov.kind !== "codex_cli" && (
                <button className="tb" onClick={fetchModels} disabled={loading || !canList} title={t("Ambil ulang daftar model")} aria-label={t("Ambil ulang daftar model")}>
                  <IRefresh />
                </button>
              )}
            </span>
          </div>
          {prov.kind === "anthropic" && (
            <label className="item">
              <span className="lbl">
                {t("Effort")}
                <small>{t("Rendah = jawaban lebih cepat")}</small>
              </span>
              <select value={prov.effort} onChange={(e) => setProv({ ...prov, effort: e.target.value })}>
                <option value="low">{t("Low — tercepat")}</option>
                <option value="medium">{t("Medium")}</option>
                <option value="high">{t("High")}</option>
              </select>
            </label>
          )}
          <div className="item">
            <span className="lbl">
              {t("Uji koneksi")}
              <small className={`selectable ${test.ok === false ? "bad" : test.ok ? "good" : ""}`}>
                {test.busy ? t("Menghubungi…") : (test.text ? t(test.text) : t("Kirim pesan singkat untuk memastikan penyedia bisa dipakai"))}
              </small>
            </span>
            <button className="push" onClick={runTest} disabled={test.busy}>
              {test.busy ? t("Menguji…") : t("Uji koneksi")}
            </button>
          </div>
        </div>
        <p className="hint">{t("Pengujian memakai pengaturan di layar ini, belum perlu disimpan.")}</p>
      </div>
    </>
  );
}
