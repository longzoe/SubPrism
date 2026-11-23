import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles/liquid-glass.css";
import { SubtitleOverlay } from "./windows/SubtitleOverlay";
import { SettingsPanel } from "./windows/SettingsPanel";

function resolveWindowMode(): "overlay" | "settings" {
  try {
    const label = getCurrentWindow().label;
    if (label === "settings") return "settings";
    if (label === "overlay") return "overlay";
  } catch {
    // Fallback for standalone browser preview
  }
  const hash = window.location.hash.replace(/^#/, "");
  return hash === "settings" ? "settings" : "overlay";
}

const mode = resolveWindowMode();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {mode === "settings" ? <SettingsPanel /> : <SubtitleOverlay />}
  </React.StrictMode>
);
