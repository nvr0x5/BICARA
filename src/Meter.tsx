import { meter } from "./api";

export function Meter({ label, rms, peak }: { label: string; rms?: number; peak?: number }) {
  const v = meter(rms);
  const p = meter(peak);
  return (
    <div className="meter">
      <span className="meter-label">{label}</span>
      <div className="meter-track">
        <div className="meter-fill" style={{ width: `${v * 100}%` }} />
        <div className="meter-peak" style={{ left: `${p * 100}%` }} />
      </div>
      <span className="meter-db">{rms && rms > 0 ? `${(20 * Math.log10(rms)).toFixed(0)} dB` : "—"}</span>
    </div>
  );
}
