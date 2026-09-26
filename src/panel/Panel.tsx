import { useEffect, useState } from "react";
import { api } from "../shared/api";
import { useSnapshot } from "../shared/useSnapshot";
import { Onboarding } from "../settings/Onboarding";
import { Settings } from "../settings/Settings";
import { MainView } from "./MainView";
import "./panel.css";

export function Panel() {
  const snap = useSnapshot();
  const [view, setView] = useState<"main" | "settings">("main");

  useEffect(() => {
    const unView = api.onPanelView((v) => setView(v === "settings" ? "settings" : "main"));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") api.closePanel();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      unView.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  if (!snap) return null;
  if (!snap.config.onboarded) return <Onboarding snap={snap} />;
  if (view === "settings") return <Settings snap={snap} onBack={() => setView("main")} />;
  return <MainView snap={snap} onOpenSettings={() => setView("settings")} />;
}
