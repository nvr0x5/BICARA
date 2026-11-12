import { useLevels } from "./api";
import { Meter } from "./Meter";

export default function Overlay() {
  const levels = useLevels();
  return (
    <main className="overlay">
      <div className="overlay-title">BICARA · Overlay</div>
      <p className="overlay-warn">
        Kalau jendela ini terlihat di layar yang kamu share / rekam, berarti <b>stealth gagal</b> untuk
        aplikasi itu.
      </p>
      <Meter label="Mereka" rms={levels.app?.rms} peak={levels.app?.peak} />
      <Meter label="Saya" rms={levels.mic?.rms} peak={levels.mic?.peak} />
    </main>
  );
}
