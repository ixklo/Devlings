import { useEffect } from "react";
import { api } from "../shared/api";
import { useSnapshot } from "../shared/useSnapshot";
import { MainView } from "./MainView";
import "./panel.css";

export function Panel() {
  const snap = useSnapshot();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") api.closePanel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  if (!snap) return null;
  return <MainView snap={snap} />;
}
