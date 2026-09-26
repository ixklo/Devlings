import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Panel } from "./panel/Panel";
import { Pet } from "./pet/Pet";
import "./styles.css";

const label = getCurrentWindow().label;
document.body.dataset.window = label;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{label === "pet" ? <Pet /> : <Panel />}</React.StrictMode>,
);
