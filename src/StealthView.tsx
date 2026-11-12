import { useEffect, useState } from "react";
import { useT } from "./i18n";
import { api } from "./api";

const MATRIX_APPS = ["Zoom", "Google Meet (Chrome)", "Microsoft Teams", "OBS / rekam layar"];
const MATRIX_MODES = ["Share layar penuh", "Share jendela"];
type Verdict = "untested" | "hidden" | "visible";
const LABEL: Record<Verdict, string> = { untested: "Belum diuji", hidden: "Tersembunyi ✓", visible: "Terlihat ✗" };
const NEXT: Record<Verdict, Verdict> = { untested: "hidden", hidden: "visible", visible: "untested" };

function load(): Record<string, Verdict> {
  try {
    return JSON.parse(localStorage.getItem("bicara.stealthMatrix") || "{}");
  } catch {
    return {};
  }
}

export default function StealthView({ hotkeys }: { hotkeys: { panic: string; assist: string } }) {
  const t = useT();
  const [stealth, setStealth] = useState(false);
  const [matrix, setMatrix] = useState<Record<string, Verdict>>(load);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.getStealth().then(setStealth);
  }, []);

  const toggle = async () => {
    try {
      setStealth(await api.setStealth(!stealth));
    } catch (e) {
      setError(String(e));
    }
  };

  const cycle = (k: string) => {
    const next = { ...matrix, [k]: NEXT[matrix[k] ?? "untested"] };
    setMatrix(next);
    try {
      localStorage.setItem("bicara.stealthMatrix", JSON.stringify(next));
    } catch {
      /* storage unavailable */
    }
  };

  return (
    <section className="card">
      <div className="card-head">
        <h2>{t("Mode siluman")}</h2>
        <button className={stealth ? "primary" : "ghost"} onClick={toggle} aria-pressed={stealth}>
          {stealth ? t("Stealth AKTIF") : t("Aktifkan stealth")}
        </button>
      </div>
      {error && <p className="notice">{t(error)}</p>}
      <p className="hint">
        {t("Saat aktif, jendela BICARA disembunyikan dari share layar & rekaman, dan tidak muncul di taskbar / Dock.")}{" "}
        <kbd>{hotkeys.panic}</kbd> {t("menyembunyikan / menampilkan semua jendela.")} <kbd>{hotkeys.assist}</kbd>{" "}
        {t("meminta jawaban AI tanpa membuka jendela.")}
      </p>
      <p className="hint">{t("Uji: aktifkan stealth → share layar di aplikasi meeting → cek preview → catat hasilnya di bawah.")}</p>
      <table className="matrix">
        <thead>
          <tr>
            <th>{t("Aplikasi")}</th>
            {MATRIX_MODES.map((m) => (
              <th key={m}>{t(m)}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {MATRIX_APPS.map((a) => (
            <tr key={a}>
              <td>{t(a)}</td>
              {MATRIX_MODES.map((m) => {
                const k = `${a}|${m}`;
                const v = matrix[k] ?? "untested";
                return (
                  <td key={m}>
                    <button className={`verdict ${v}`} onClick={() => cycle(k)}>
                      {t(LABEL[v])}
                    </button>
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
