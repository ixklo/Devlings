/** True inside a Tauri webview; false in a plain browser (preview/mock mode) and in tests. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export type WindowLabel = "pet" | "settings";

/** The window this page is rendering. Browser preview picks it with `?window=pet|settings`. */
export function windowLabel(): WindowLabel {
  let label: string | undefined;
  if (isTauri()) {
    const internals = (window as unknown as { __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string } } } })
      .__TAURI_INTERNALS__;
    label = internals?.metadata?.currentWindow?.label;
  } else {
    label = new URLSearchParams(window.location.search).get("window") ?? undefined;
  }
  return label === "settings" ? "settings" : "pet";
}
