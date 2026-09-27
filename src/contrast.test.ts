import { describe, expect, it } from "vitest";
import css from "./styles.css?raw";

/**
 * WCAG AA contrast for Devlings' colour tokens (styles.css), light and dark: 4.5:1 for text, 3:1 for
 * UI parts (focus rings, control edges, status glyphs). Each pair is a foreground and background that
 * actually meet somewhere in the UI; change a token and this fails if it drops below AA.
 */

type Rgb = [number, number, number];

function hexToRgb(hex: string): Rgb {
  let h = hex.slice(1);
  if (h.length === 3) h = [...h].map((c) => c + c).join("");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as Rgb;
}

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

/** oklch → linear sRGB (Björn Ottosson's OKLab matrices), clipped to the sRGB gamut like a browser. */
function oklchToLinear(l: number, c: number, hDeg: number): Rgb {
  const h = (hDeg * Math.PI) / 180;
  const a = c * Math.cos(h);
  const b = c * Math.sin(h);
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const rgb: Rgb = [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ];
  return rgb.map((v) => Math.min(1, Math.max(0, v))) as Rgb;
}

function linearRgb(color: string): Rgb {
  const c = color.trim();
  if (/^#[0-9a-f]{3}([0-9a-f]{3})?$/i.test(c)) return hexToRgb(c).map(toLinear) as Rgb;
  const m = c.match(/^oklch\(\s*([\d.]+)(%?)\s+([\d.]+)\s+([\d.]+)\s*\)$/);
  if (!m) throw new Error(`Can't read colour "${color}"`);
  return oklchToLinear(m[2] ? Number(m[1]) / 100 : Number(m[1]), Number(m[3]), Number(m[4]));
}

function luminance(color: string): number {
  const [r, g, b] = linearRgb(color);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

function declarations(block: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of block.matchAll(/--([\w-]+):\s*([^;]+);/g)) out[m[1]] = m[2].trim();
  return out;
}

function themes(source: string) {
  const light = source.match(/:root\s*{([^}]*)}/);
  const dark = source.match(/@media \(prefers-color-scheme: dark\)\s*{\s*:root\s*{([^}]*)}/);
  if (!light || !dark) throw new Error("styles.css: couldn't find the light and dark token blocks");
  const l = declarations(light[1]);
  return { light: l, dark: { ...l, ...declarations(dark[1]) } };
}

const TEXT = 4.5;
const UI = 3;

/** [foreground, background, minimum, where]. Names are tokens; `#…` is a literal colour. */
const PAIRS: [string, string, number, string][] = [
  ["ink", "surface", TEXT, "card and body text"],
  ["ink", "bg", TEXT, "settings window text"],
  ["ink", "fill-1", TEXT, "your message bubble, hovered menu item"],
  ["ink", "code-bg", TEXT, "code blocks, approval details"],
  ["ink", "wait-soft", TEXT, "approval row in the mini chat"],
  ["ink-2", "surface", TEXT, "card lines, descriptions, icon buttons"],
  ["ink-2", "surface-raised", TEXT, "segmented control, menus"],
  ["ink-2", "bg", TEXT, "onboarding lede"],
  ["ink-2", "fill-1", TEXT, "Ask label, untrusted-folder row"],
  ["ink-2", "fill-2", TEXT, "card dismiss button"],
  ["ink-2", "wait-soft", TEXT, "approval row description"],
  ["ink-3", "surface", TEXT, "muted labels, times, placeholders"],
  ["ink-3", "surface-raised", TEXT, "menu headings and details"],
  ["ink-3", "bg", TEXT, "hints and step counts"],
  ["ink-3", "fill-1", TEXT, "reply placeholder, skipped list"],
  ["ink-3", "fill-2", TEXT, "disabled send button"],
  ["ink-3", "wait-soft", TEXT, "approval row tool name and rule line"],
  ["ink-3", "code-bg", TEXT, "muted text on code"],
  ["accent-ink", "surface", TEXT, "links"],
  ["accent-ink", "fill-1", TEXT, "links in banners"],
  ["accent-ink", "bg", TEXT, "links on the window background"],
  ["wait-ink", "surface", TEXT, "Needs you status"],
  ["wait-ink", "fill-1", TEXT, "untrusted-folder icon"],
  ["wait-ink", "wait-soft", TEXT, "approval row accents"],
  ["ok-ink", "surface", TEXT, "Done status"],
  ["ok-ink", "surface-raised", TEXT, "success toast icon"],
  ["ok-ink", "fill-1", TEXT, "trusted-folder line"],
  ["err-ink", "surface", TEXT, "errors, Blocked status, Remove button"],
  ["err-ink", "surface-raised", TEXT, "error toast"],
  ["err-ink", "bg", TEXT, "errors on the window background"],
  ["err-ink", "err-soft", TEXT, "inline errors, risk badges"],
  ["err-ink", "wait-soft", TEXT, "too-long note in an approval row"],
  ["on-primary", "primary", TEXT, "primary buttons, send, count badge"],
  ["on-primary", "primary-hover", TEXT, "primary buttons on hover"],
  ["on-color", "err-ink", TEXT, "red count badge"],
  ["on-wait", "wait", TEXT, "amber count badge"],
  ["code-comment", "code-bg", TEXT, "code comments"],
  ["code-keyword", "code-bg", TEXT, "code keywords"],
  ["code-string", "code-bg", TEXT, "code strings"],
  ["code-number", "code-bg", TEXT, "code numbers"],
  ["code-title", "code-bg", TEXT, "code titles"],
  ["code-variable", "code-bg", TEXT, "code variables"],
  ["code-deletion", "code-bg", TEXT, "code deletions"],
  ["accent", "surface", UI, "focus ring on cards, selected pet"],
  ["accent", "surface-raised", UI, "focus ring in menus"],
  ["accent", "bg", UI, "focus ring on the window background"],
  ["accent", "fill-1", UI, "focus ring on filled controls"],
  ["accent", "wait-soft", UI, "focus ring in an approval row"],
  ["on-color", "ok", UI, "check glyph on green"],
  ["on-color", "err", UI, "cross glyph on red"],
  ["on-color", "accent", UI, "check glyph on the selected pet"],
  ["ok", "surface", UI, "Done status disc"],
  ["err", "surface", UI, "Blocked status disc"],
  ["wait-ink", "surface", UI, "Needs you dot outline"],
  ["ink-2", "surface", UI, "working spinner"],
  ["line-control", "surface", UI, "switch and text box edges"],
  ["line-control", "surface-raised", UI, "selected segment outline"],
  ["line-control", "fill-1", UI, "selected segment against its track"],
  ["primary", "surface", UI, "switch on"],
];

describe("colour contrast (WCAG AA)", () => {
  const t = themes(css);
  for (const [mode, tokens] of Object.entries(t)) {
    describe(mode, () => {
      it.each(PAIRS)("%s on %s ≥ %s (%s)", (fg, bg, min) => {
        const color = (name: string) => {
          const v = name.startsWith("#") ? name : tokens[name];
          if (!v) throw new Error(`No --${name} token in ${mode} mode`);
          return v;
        };
        expect(contrast(color(fg), color(bg))).toBeGreaterThanOrEqual(min);
      });
    });
  }

  it("computes known ratios", () => {
    expect(contrast("#000", "#fff")).toBeCloseTo(21, 5);
    expect(contrast("#777777", "#ffffff")).toBeCloseTo(4.48, 2);
    expect(luminance("oklch(1 0 0)")).toBeCloseTo(1, 3);
  });
});

describe("focus and motion rules", () => {
  it("draws a 2 px accent focus ring on everything focused from the keyboard", () => {
    expect(css).toMatch(/:focus-visible\s*{\s*outline:\s*2px solid var\(--accent\);/);
  });

  it("turns animations and transitions off for reduced motion", () => {
    const block = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));
    expect(block).toMatch(/animation-duration:\s*0\.01ms !important/);
    expect(block).toMatch(/animation-iteration-count:\s*1 !important/);
    expect(block).toMatch(/transition-duration:\s*0\.01ms !important/);
  });
});
