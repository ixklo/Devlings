import { useEffect, useState } from "react";
import { api } from "./api";
import type { Snapshot } from "./types";

export function useSnapshot(): Snapshot | null {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  useEffect(() => {
    let alive = true;
    api.getSnapshot().then((s) => alive && setSnap(s));
    const unlisten = api.onSnapshot(setSnap);
    return () => {
      alive = false;
      unlisten.then((f) => f());
    };
  }, []);
  return snap;
}
