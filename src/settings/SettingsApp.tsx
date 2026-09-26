import { useEffect, useState } from "react";
import { api } from "../shared/api";
import type { SettingsView } from "../shared/types";
import { useSnapshot } from "../shared/useSnapshot";
import { Onboarding } from "./Onboarding";
import { Settings } from "./Settings";
import "./settings.css";

/** The settings window: onboarding on first run, Settings afterwards. */
export function SettingsApp() {
  const snap = useSnapshot();
  const [view, setView] = useState<SettingsView | null>(null);

  useEffect(() => {
    const unlisten = api.onSettingsView(setView);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  if (!snap) return null;
  // The first-run `settings-view` event can fire before this page listens, so
  // an unfinished onboarding always wins.
  const effective: SettingsView = !snap.config.onboarded ? "onboarding" : (view ?? "settings");
  if (effective === "onboarding") return <Onboarding snap={snap} onDone={() => setView("settings")} />;
  return <Settings snap={snap} />;
}
