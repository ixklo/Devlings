// Species "ghost" (Wisp). Personality: it floats, bobbing gently in every
// mood, and fades a little (paler, then see-through) while it idles.
import { compose, dropRows, dupRows, shear } from '../../lib/engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from '../../lib/palettes.mjs';
import { moodRows, part, group } from '../../lib/moods.mjs';
import * as S from './sprites.mjs';

const HOVER = 43; // lowest fill row of the hem: it floats a little above the others' floor
const BX = 11;

function squashed(s, squash) {
  if (squash > 0) return dropRows(s, S.SQUASH_ROWS.slice(0, squash));
  if (squash < 0) return dupRows(s, S.SQUASH_ROWS.slice(0, -squash));
  return s;
}

function arm(pose, side, top) {
  const p = S.ARM_POSES[pose];
  if (side === 'l') return group(part(p.s, BX + p.x, top + p.y));
  return group(part(S.rightOf(p.s), BX + S.BODY_W - p.x - p.s.w, top + p.y));
}

// Fading: level 1 pales the outline and shadows; level 2 also dissolves the
// hem, letting the desktop show through it (hard pixels, no blending).
const BODY_ROLES = new Set(['h', 'b', 's', 'd', 'o', 'c']);
const PALER = { o: 'c', d: 's' };
function fade(layer, level, bottom) {
  if (!level) return layer;
  const out = layer.clone();
  for (let y = 0; y < out.h; y++) {
    const fromBottom = bottom + 1 - y; // the outline row under the hem is 0
    for (let x = 0; x < out.w; x++) {
      const v = out.get(x, y);
      if (!BODY_ROLES.has(v)) continue;
      const gone = level >= 2 && ((fromBottom <= 3 && (x + y) % 2 === 0) || (fromBottom <= 6 && x % 2 === 0 && y % 2 === 0));
      out.set(x, y, gone ? null : (PALER[v] ?? v));
    }
  }
  return out;
}

export function front(o = {}) {
  const lift = (o.lift ?? 0) + (o.bob ?? 0);
  const body = squashed(S.BODY, o.squash ?? 0);
  const bottom = HOVER - lift;
  const top = bottom - body.h + 1;
  const hx = BX + (o.lean ?? 0);
  const fx = hx + (o.eyeDx ?? 0);
  const eyeY = top + 9 + (o.eyeDy ?? 0);
  const eyes = S.EYES[o.eyes ?? 'open'];
  const curl = o.droop ? S.CURL.droop : S.CURL.up;
  const geo = { top, bottom, fx: hx, fy: top, hx, eyeY, lift };
  const extra = o.props ? o.props(geo) : {};
  const groups = [
    ...(extra.back ?? []),
    group(
      part(curl, hx + 10, top - curl.h + 1),
      part(body, BX, top),
      part(eyes.l, fx + 6, eyeY),
      part(eyes.r, fx + 16, eyeY),
      part(S.BLUSH, fx + 3, top + 14),
      part(S.BLUSH, fx + 21, top + 14),
      part(S.MOUTH, fx + 11, top + 14),
      ...(extra.face ?? []),
    ),
    ...(extra.mid ?? []),
    arm(o.armL ?? 'rest', 'l', top),
    arm(o.armR ?? 'rest', 'r', top),
    ...(extra.front ?? []),
  ];
  return fade(compose(groups, OUTLINE_OF, LINE_ROLES), o.fade ?? 0, bottom);
}

/** Side view, drifting right: a bob and a lean, the tail trailing. */
export function side(o = {}) {
  const lift = o.lift ?? 0;
  const body = shear(S.SIDE_BODY, o.lean ?? 0);
  const x0 = BX - body.ox;
  const top = HOVER - lift - body.h + 1;
  // the face sits on the front of the dome, which the lean pushes forward
  const fx = BX + Math.round(8 * (o.lean ?? 0)) + 14;
  const groups = [
    group(
      part(S.CURL.up, fx + 1, top - 2),
      part(body, x0, top),
      part(S.EYES.open.l, fx, top + 8),
      part(S.EYES.open.r, fx + 6, top + 8),
      part(S.BLUSH, fx - 2, top + 13),
      part(S.MOUTH, fx + 3, top + 13),
    ),
    group(part(S.SIDE_ARM, fx - 9, top + 15 + (o.armDy ?? 0))),
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

function run() {
  return [
    side({ lift: 1, lean: 0.1, armDy: 0 }),
    side({ lift: 2, lean: 0.16, armDy: -1 }),
    side({ lift: 1, lean: 0.1, armDy: 0 }),
    side({ lift: 0, lean: 0.04, armDy: 1 }),
  ];
}

const FLOAT = [0, 1, 1, 0];
const float = (i) => ({ bob: FLOAT[i % 4] });

const rows = moodRows({
  front,
  run,
  anchors: {
    ask: [34, 3],
    puff: [31, 3],
    cloud: [15, 4],
    drops: [19, 28],
    laptop: (g) => [15, g.bottom - 10],
    lens: (g) => [g.hx + 14, g.eyeY - 3],
  },
  touch: {
    idle: (i) => ({ fade: [0, 1, 2, 2, 1, 0][i], bob: [0, 0, 1, 1, 1, 0][i] }),
    waving: float,
    failed: float,
    waiting: float,
    working: float,
    review: float,
  },
});

export default { name: 'ghost', rows };
