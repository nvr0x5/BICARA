import { meter } from "./api";

/** Compact level meter for the toolbar (transform-animated, no layout work). */
export function Meter({ rms, peak }: { rms?: number; peak?: number }) {
  return (
    <span className="meter" role="meter" aria-valuemin={0} aria-valuemax={1} aria-valuenow={meter(rms)}>
      <span className="meter-fill" style={{ transform: `scaleX(${meter(rms)})` }} />
      <span className="meter-peak" style={{ transform: `translateX(${meter(peak) * 100}%)` }} />
    </span>
  );
}
