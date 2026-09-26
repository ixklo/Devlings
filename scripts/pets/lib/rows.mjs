// Frame composition for every atlas row.
import { compose, dropRows, dupRows, flipX } from './engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from './palettes.mjs';
import * as S from './sprites.mjs';

// Layout on the 48x52 art grid.
export const BX = 9; // body left (fill)
export const BY = 13; // body top (fill) in the neutral pose
const BODY_BOTTOM = BY + S.BODY_H - 1; // 40
const FEET_Y = 41;
const BRANCH_X = 4;
const BRANCH_Y = 43 - S.BRANCH_TOP;

const part = (s, x, y) => ({ s, x, y });
const group = (...parts) => ({ parts });

function squashed(body, squash) {
  if (squash > 0) return dropRows(body, S.SQUASH_ROWS.slice(0, squash));
  if (squash < 0) return dupRows(body, S.SQUASH_ROWS.slice(0, -squash));
  return body;
}

/**
 * Front-facing bird.
 *  lift    whole bird (incl. feet) moves up by this many px (jumping)
 *  squash  body rows removed (>0) or added (<0); the bottom stays planted
 *  lean    face and tuft shift in x (head turn / wobble)
 *  props   (geo) => { back, mid, front } extra groups placed from geometry
 */
export function front(o = {}) {
  const lift = o.lift ?? 0;
  const body = squashed(S.BODY, o.squash ?? 0);
  const bx = BX;
  const bottom = BODY_BOTTOM - lift;
  const top = bottom - body.h + 1;
  const fx = bx + (o.lean ?? 0);
  const fy = top + (o.faceDy ?? 0);
  const ex = fx + (o.eyeDx ?? 0);
  const ey = fy + (o.eyeDy ?? 0);
  const eyes = S.EYES[o.eyes ?? 'open'];
  const tuft = S.TUFT[o.tuft ?? 'up'];
  const wl = S.WING_POSES[o.wingL ?? 'rest'];
  const wr = S.rightWing(o.wingR ?? 'rest');
  const geo = { bx, top, bottom, fx, fy, ex, ey, lift };
  const extra = o.props ? o.props(geo) : {};

  const groups = [
    ...(extra.back ?? []),
    group(part(S.TAIL, bx + 27, bottom - 7)),
    o.branch === false ? null : group(part(S.BRANCH, BRANCH_X, BRANCH_Y)),
    group(
      part(tuft, fx + 11, fy - tuft.h + 2),
      part(body, bx, top),
      o.eyes === 'none' ? null : part(eyes.l, ex + 7, ey + 7),
      o.eyes === 'none' ? null : part(eyes.r, ex + 18, ey + 7),
      part(S.BLUSH, ex + 4, ey + 13),
      part(S.BLUSH, ex + 24, ey + 13),
      ...(extra.face ?? []),
    ),
    group(part(S.BEAK, ex + 13, ey + 12)),
    ...(extra.mid ?? []),
    group(part(wl.s, bx + wl.x, top + wl.y)),
    group(part(wr.s, bx + wr.x, top + wr.y)),
    o.feet === false ? null : group(part(S.FOOT, 17, FEET_Y - lift), part(S.FOOT, 28, FEET_Y - lift)),
    ...(extra.front ?? []),
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

/** Side view, facing right. */
export function side(o = {}) {
  const lift = o.lift ?? 0;
  const body = squashed(S.SIDE_BODY, o.squash ?? 0);
  const bx = BX;
  const bottom = BODY_BOTTOM - lift;
  const top = bottom - body.h + 1;
  const wing = S.SIDE_WING[o.wing ?? 'down'];
  const tuft = flipX(S.TUFT[o.tuft ?? 'up']);
  const feet = S.SIDE_FEET[o.feet ?? 'planted'];
  const groups = [
    group(part(S.SIDE_TAIL, bx - 6, top + 14)),
    group(
      part(tuft, bx + 11, top - tuft.h + 2),
      part(body, bx, top),
      part(S.EYES[o.eyes ?? 'open'].l, bx + 20, top + 7),
      part(S.BLUSH, bx + 22, top + 13),
    ),
    group(part(S.SIDE_BEAK, bx + 28, top + 11)),
    group(part(wing.s, bx + wing.x, top + wing.y)),
    group(part(feet, bx + 11, FEET_Y - lift)),
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

const mirror = (layer) => {
  const out = layer.clone();
  for (let y = 0; y < layer.h; y++) {
    for (let x = 0; x < layer.w; x++) out.set(x, y, layer.get(layer.w - 1 - x, y));
  }
  return out;
};

// ------------------------------------------------------------------ rows
function runRight() {
  const hop = [
    { lift: 3, squash: -1, wing: 'up', feet: 'tucked' },
    { lift: 5, wing: 'mid', feet: 'tucked' },
    { lift: 2, wing: 'down', feet: 'planted' },
    { lift: 0, squash: 1, wing: 'down', feet: 'planted' },
  ];
  return [...hop, ...hop].map((f) => side(f));
}

function working() {
  const laptop = (dots) => () => ({
    // screen light spilling onto the belly just above the lid
    mid: [{ parts: [part(S.SCREEN_GLOW, 16, 32)], outline: false }],
    front: [group(part(S.LAPTOP, 15, 33))],
    back: [group(part(S.PUFFS[dots], 31, 3))],
  });
  const f = (wingL, wingR, squash, dots) =>
    front({ eyeDy: 1, wingL, wingR, squash, props: laptop(dots) });
  return [
    f('type', 'type2', 0, 1),
    f('type2', 'type', 1, 2),
    f('type', 'type2', 0, 3),
    f('type2', 'type', 1, 3),
    f('type', 'type2', 0, 0),
    f('type2', 'type', 1, 1),
  ];
}

function waiting() {
  const ask = (dy) => () => ({ back: [group(part(S.ASK, 33, 3 + dy))] });
  const base = { wingL: 'up', eyes: 'up', eyeDy: -1 };
  return [
    front({ ...base, props: ask(0) }),
    front({ ...base, squash: 1, props: ask(0) }),
    front({ ...base, squash: -1, props: ask(-1) }),
    front({ ...base, props: ask(-1) }),
    front({ ...base, eyes: 'blink', props: ask(0) }),
    front({ ...base, props: ask(0) }),
  ];
}

function failed() {
  const lean = [0, -1, -1, 0, 0, 1, 1, 0];
  const cloudDy = [0, 0, 1, 1, 0, 0, 1, 1];
  // two rain streaks, out of phase; drops slip behind the head
  const dropA = [11, 13, 15, null, 11, 13, 15, null];
  const dropB = [15, null, 11, 13, 15, null, 11, 13];
  return lean.map((l, i) =>
    front({
      squash: 2,
      tuft: 'droop',
      wingL: 'droop',
      wingR: 'droop',
      eyes: 'teary',
      lean: l,
      props: () => ({
        back: [
          group(part(S.CLOUD, 15, 2 + cloudDy[i])),
          dropA[i] == null ? null : group(part(S.DROP, 19, dropA[i] + cloudDy[i])),
          dropB[i] == null ? null : group(part(S.DROP, 28, dropB[i] + cloudDy[i])),
        ],
      }),
    }),
  );
}

function review() {
  const scan = [-1, -1, 0, 1, 1, 0];
  const nod = [0, 0, 0, 0, 1, 0];
  return scan.map((dx, i) =>
    front({
      eyeDx: dx,
      squash: nod[i],
      wingR: 'hold',
      props: (g) => {
        // the lens stays in the wing; the eye behind it scans with the face
        const lx = g.fx + 16;
        const ly = g.fy + 5;
        return {
          mid: [
            group(part(S.HANDLE, lx + 8, ly + 8)),
            group(part(S.LENS, lx, ly), part(S.BIG_EYE, lx + 2 + dx, ly + 1)),
          ],
        };
      },
    }),
  );
}

export const ROW_SPECS = [
  {
    name: 'idle',
    frames: () => [
      front(),
      front({ eyes: 'half' }),
      front({ eyes: 'blink' }),
      front({ eyes: 'half' }),
      front({ squash: 1, tuft: 'sway' }),
      front({ squash: 1 }),
    ],
  },
  { name: 'running-right', frames: runRight },
  { name: 'running-left', frames: () => runRight().map(mirror) },
  {
    name: 'waving',
    frames: () => [
      front({ wingL: 'flat' }),
      front({ wingL: 'up', eyes: 'happy', lean: -1 }),
      front({ wingL: 'diag', eyes: 'happy', lean: -1 }),
      front({ wingL: 'flat', eyes: 'happy' }),
    ],
  },
  {
    name: 'jumping',
    frames: () => [
      front({ squash: 2, eyes: 'blink' }),
      front({ lift: 4, squash: -1, wingL: 'diag', wingR: 'diag' }),
      front({ lift: 6, wingL: 'flat', wingR: 'flat', eyes: 'happy' }),
      front({ lift: 3, wingL: 'up', wingR: 'up' }),
      front({ squash: 1 }),
    ],
  },
  { name: 'failed', frames: failed },
  { name: 'waiting', frames: waiting },
  { name: 'running', frames: working },
  { name: 'review', frames: review },
];
