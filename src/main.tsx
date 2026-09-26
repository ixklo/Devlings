import React from "react";
import ReactDOM from "react-dom/client";
import { setTransport } from "./shared/api";
import { isTauri, windowLabel } from "./shared/env";
import "./styles.css";

async function boot() {
  if (!isTauri()) {
    // Plain browser: render against the fake backend (see shared/mock.ts).
    const { createMockTransport } = await import("./shared/mock");
    setTransport(createMockTransport());
  }
  const label = windowLabel();
  document.body.dataset.window = label;
  document.title = label === "pet" ? "Perch" : "Perch Settings";
  const App =
    label === "pet"
      ? (await import("./pet/PetApp")).PetApp
      : (await import("./settings/SettingsApp")).SettingsApp;
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void boot();
