import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Overlay from "./Overlay";
import SettingsWindow from "./SettingsWindow";
import { platform } from "./api";
import "./App.css";

document.documentElement.dataset.platform = platform;

/** Last-resort error display so a crash never leaves an empty window. */
class Boundary extends React.Component<{ children: React.ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <pre className="selectable" style={{ padding: 24, whiteSpace: "pre-wrap", color: "var(--red)" }}>
        {String(this.state.error)}
        {"\n"}
        {this.state.error.stack}
      </pre>
    );
  }
}
const view = new URLSearchParams(window.location.search).get("view");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode><Boundary>{view === "overlay" ? <Overlay /> : view === "settings" ? <SettingsWindow /> : <App />}</Boundary></React.StrictMode>,
);
