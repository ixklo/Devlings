import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Pet } from "./pet/Pet";
import "./styles.css";

const label = getCurrentWindow().label;
document.body.dataset.window = label;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{label === "pet" ? <Pet /> : <p>Panel coming in Task 13</p>}</React.StrictMode>,
);
