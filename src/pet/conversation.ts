import type { ChatTurn, PetEvent } from "../shared/types";

function finalize(turns: ChatTurn[]): ChatTurn[] {
  const last = turns[turns.length - 1];
  if (last?.role === "assistant" && last.pending) {
    return [...turns.slice(0, -1), { role: "assistant", text: last.text }];
  }
  return turns;
}

export function applyAskEvent(turns: ChatTurn[], ev: PetEvent): ChatTurn[] {
  const last = turns[turns.length - 1];
  switch (ev.kind) {
    case "reply_delta":
      if (last?.role === "assistant" && last.pending) {
        return [...turns.slice(0, -1), { ...last, text: last.text + (ev.text ?? "") }];
      }
      return [...turns, { role: "assistant", text: ev.text ?? "", pending: true }];
    case "step":
      return finalize(turns);
    case "blocked":
      return [...finalize(turns), { role: "note", text: ev.label ?? "Blocked" }];
    case "done":
      if (last?.role === "assistant" && last.pending) {
        return [...turns.slice(0, -1), { role: "assistant", text: last.text || ev.text || "" }];
      }
      return ev.text && last?.role !== "assistant" ? [...turns, { role: "assistant", text: ev.text }] : turns;
    case "failed":
      return [...finalize(turns), { role: "note", text: [ev.label, ev.text].filter(Boolean).join(" — ") }];
    case "ended":
      return ev.label ? [...finalize(turns), { role: "note", text: ev.label }] : finalize(turns);
    default:
      return turns;
  }
}
