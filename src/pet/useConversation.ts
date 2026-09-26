import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../shared/api";
import { samePath } from "../shared/paths";
import type { ChatTurn } from "../shared/types";
import { applyAskEvent } from "./conversation";

function dropLastUserTurn(turns: ChatTurn[], text: string): ChatTurn[] {
  for (let i = turns.length - 1; i >= 0; i--) {
    if (turns[i].role === "user" && turns[i].text === text) return [...turns.slice(0, i), ...turns.slice(i + 1)];
  }
  return turns;
}

/**
 * The Ask conversation for one project. Lives at the app root so streamed
 * pet-events are never missed while the thread view is closed or mounting.
 */
export function useConversation() {
  const [project, setProject] = useState<string | null>(null);
  const [turns, setTurns] = useState<ChatTurn[]>([]);
  const [loading, setLoading] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const projectRef = useRef<string | null>(null);
  const loadToken = useRef(0);

  useEffect(() => {
    const unlisten = api.onPetEvent((ev) => {
      const current = projectRef.current;
      if (ev.source !== "ask" || !current || !samePath(ev.project, current)) return;
      setTurns((t) => applyAskEvent(t, ev));
      if (ev.sessionId) setSessionId(ev.sessionId);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const open = useCallback((path: string) => {
    if (projectRef.current && samePath(projectRef.current, path)) return;
    projectRef.current = path;
    setProject(path);
    setTurns([]);
    setSessionId(null);
    const token = ++loadToken.current;
    setLoading(true);
    api
      .loadConversation(path)
      .then((loaded) => {
        // Anything streamed or sent while loading goes after the saved history.
        if (token === loadToken.current) setTurns((live) => [...loaded, ...live]);
      })
      .catch(() => {})
      .finally(() => {
        if (token === loadToken.current) setLoading(false);
      });
  }, []);

  const send = useCallback(async (text: string) => {
    const path = projectRef.current;
    if (!path) throw new Error("Pick a project first.");
    setTurns((t) => [...t, { role: "user", text }]);
    try {
      await api.ask(path, text);
    } catch (e) {
      setTurns((t) => dropLastUserTurn(t, text));
      throw e;
    }
  }, []);

  const reset = useCallback(async () => {
    const path = projectRef.current;
    if (!path) return;
    await api.newConversation(path);
    loadToken.current++;
    setTurns([]);
    setSessionId(null);
    setLoading(false);
  }, []);

  return { project, turns, loading, sessionId, open, send, reset };
}

export type Conversation = ReturnType<typeof useConversation>;
