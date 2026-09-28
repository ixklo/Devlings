import { afterEach, describe, expect, it, vi } from "vitest";
import { hiddenCount } from "../pet/hiddenCount";
import type { Transport } from "./api";
import { SCENES, sceneFor } from "./mockScenes";
import type { PetInfo, Snapshot } from "./types";

/** A fresh browser-preview backend for `search` (the module reads the query string once, when it loads). */
async function preview(search: string): Promise<Transport> {
  window.history.replaceState(null, "", `/${search}`);
  vi.resetModules();
  const { createMockTransport } = await import("./mock");
  return createMockTransport();
}

async function snapshot(t: Transport): Promise<Snapshot> {
  const pending = t.invoke<Snapshot>("get_snapshot");
  await vi.advanceTimersByTimeAsync(50);
  return pending;
}

afterEach(() => {
  vi.useRealTimers();
  window.history.replaceState(null, "", "/");
});

describe("browser preview sprites", () => {
  it("shows the bundled pets' real spritesheets, which the dev server serves from the source tree", async () => {
    const t = await preview("?window=pet");
    await expect(t.invoke("get_pet_sprite", { id: "fox" })).resolves.toBe("/src-tauri/pets/fox/spritesheet.png");
    await expect(t.invoke("get_pet_sprite", { id: "perch" })).resolves.toBe("/src-tauri/pets/perch/spritesheet.png");
  });
});

describe("preview scenes (?scene=), used for the README and website images", () => {
  const MADE_UP = ["weather-app", "blog", "recipe-box", "portfolio"];

  it.each(SCENES)("%s only shows made-up projects under a placeholder home folder", (name) => {
    const scene = sceneFor(name, Date.now(), 60);
    expect(scene).not.toBeNull();
    const everything = JSON.stringify(scene);
    expect(everything).not.toMatch(/devlings|perch/i);
    const paths = [
      ...scene!.projects.map((p) => p.path),
      ...scene!.threads.map((t) => t.project),
      ...scene!.approvals.map((a) => a.project),
      ...scene!.timeline.flatMap((s) => s.threads?.map((t) => t.project) ?? []),
    ];
    for (const p of paths) expect(p).toMatch(/^C:\\Users\\you\\code\\[a-z-]+$/);
    const names = [...scene!.projects.map((p) => p.name), ...paths.map((p) => p.split("\\").pop())];
    for (const n of names) expect(MADE_UP).toContain(n);
  });

  it("is off unless asked for", () => {
    expect(sceneFor(null, Date.now(), 60)).toBeNull();
    expect(sceneFor("nope", Date.now(), 60)).toBeNull();
  });

  it("lists only the nine built-in pets", async () => {
    vi.useFakeTimers();
    const t = await preview("?scene=cards");
    const pending = t.invoke<PetInfo[]>("list_pets");
    await vi.advanceTimersByTimeAsync(50);
    const pets = await pending;
    expect(pets.map((p) => p.displayName)).toEqual(["Perch", "Ember", "Plum", "Pip", "Miso", "Nori", "Bean", "Bolt", "Wisp"]);
  });

  it("cards: a finished Ask and two running sessions, and the pet inspecting the result", async () => {
    vi.useFakeTimers();
    const snap = await snapshot(await preview("?scene=cards"));
    expect(snap.threads.map((t) => [t.projectName, t.status])).toEqual([
      ["weather-app", "ready"],
      ["blog", "running"],
      ["portfolio", "running"],
    ]);
    expect(snap.approvals).toEqual([]);
    expect(snap.petState).toBe("ready");
  });

  it("approval: one permission request with Deny, Allow and Always allow", async () => {
    vi.useFakeTimers();
    const snap = await snapshot(await preview("?scene=approval"));
    expect(snap.approvals).toHaveLength(1);
    expect(snap.approvals[0]).toMatchObject({ projectName: "recipe-box", toolName: "Bash", canAlwaysAllow: true });
    expect(snap.petState).toBe("needs_input");
  });

  it("chat: opening the thread view shows its own Ask, which streams a reply and finishes", async () => {
    vi.useFakeTimers();
    const t = await preview("?scene=chat&open=thread");
    const opened = vi.fn();
    await t.listen("pet-open", opened);
    const replies: string[] = [];
    await t.listen<{ kind: string; text?: string }>("pet-event", (e) => e.kind === "reply_delta" && replies.push(e.text ?? ""));
    const history = t.invoke<{ role: string }[]>("load_conversation", { project: "C:\\Users\\you\\code\\weather-app" });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(opened).toHaveBeenCalledWith({ view: "thread", sessionId: "ask-weather-app" });
    expect((await history).map((turn) => turn.role)).toEqual(["user"]);
    expect(replies.join("")).toContain("local date");
    const snap = await snapshot(t);
    expect(snap.running).toEqual([]);
    expect(snap.threads[0]).toMatchObject({ projectName: "weather-app", status: "ready" });
  });

  it("demo: idle, then working, then done with a card, then the cards tucked away behind a count", async () => {
    vi.useFakeTimers();
    const t = await preview("?scene=demo");
    const at = async (ms: number) => {
      await vi.advanceTimersByTimeAsync(ms);
      return snapshot(t);
    };
    const start = await at(0);
    expect(start.threads).toEqual([]);
    expect(start.petState).toBe("idle");

    const working = await at(2_000);
    expect(working.petState).toBe("running");
    expect(working.threads.map((x) => x.projectName)).toContain("weather-app");

    const done = await at(4_000);
    expect(done.petState).toBe("ready");
    expect(done.threads[0]).toMatchObject({ projectName: "weather-app", status: "ready", unread: true });
    expect(done.config.threadsCollapsed).toBe(false);

    const tucked = await at(4_000);
    expect(tucked.config.threadsCollapsed).toBe(true);
    expect(hiddenCount(tucked.threads, tucked.approvals)).toEqual({ threads: 2, tone: "neutral", count: 2 });
  });
});
