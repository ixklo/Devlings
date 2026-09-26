import { vi } from "vitest";
import { setTransport, type Transport } from "../shared/api";

type Handler = (args: Record<string, unknown> | undefined) => unknown;

/**
 * Swaps the host transport for a recorder. Handlers run per command name;
 * a handler that throws makes the command reject, like a Rust `Err`.
 */
export function fakeTransport(handlers: Record<string, Handler> = {}) {
  const calls: [string, Record<string, unknown> | undefined][] = [];
  const listeners = new Map<string, Set<(p: unknown) => void>>();
  const transport: Transport = {
    invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
      calls.push([cmd, args]);
      const h = handlers[cmd];
      return h ? h(args) : undefined;
    }) as Transport["invoke"],
    listen: vi.fn(async (event: string, cb: (p: never) => void) => {
      const set = listeners.get(event) ?? new Set();
      const entry = (p: unknown) => cb(p as never);
      set.add(entry);
      listeners.set(event, set);
      return () => void set.delete(entry);
    }) as Transport["listen"],
    startDragging: vi.fn(async () => {}),
    onMoved: vi.fn(async () => () => {}),
    chooseFolder: vi.fn(async () => null),
    chooseFile: vi.fn(async () => null),
    openUrl: vi.fn(async () => {}),
    appVersion: vi.fn(async () => "0.2.0"),
  };
  setTransport(transport);
  const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((cb) => cb(payload));
  const commands = () => calls.map(([cmd]) => cmd);
  return { transport, calls, commands, emit };
}
