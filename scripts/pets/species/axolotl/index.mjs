// Species "axolotl" (Nori). Personality: the gill frills flutter while it waits.
import { compose, shear, recolor } from '../../lib/engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from '../../lib/palettes.mjs';
import { moodRows, part, group } from '../../lib/moods.mjs';
import { makeSitter } from '../../lib/sitter.mjs';
import * as S from './sprites.mjs';

const FLOOR = 45;
const TX = 14;
const HX = 9;

export const front = makeSitter({
  floor: FLOOR,
  head: S.HEAD,
  headX: HX,
  overlap: 3,
  ears: {
    up: { s: S.FRILLS.up, x: -6, y: 1 },
    flutter: { s: S.FRILLS.flutter, x: -6, y: 0 },
    droop: { s: S.FRILLS.droop, x: -6, y: 3 },
  },
  eyes: S.EYES,
  eyeAt: { lx: 5, rx: 21, y: 5 },
  face: [
    { s: S.BLUSH, x: 3, y: 9 },
    { s: S.BLUSH, x: 25, y: 9 },
    { s: S.SMILE, x: 10, y: 8 },
  ],
  torso: S.TORSO,
  torsoX: TX,
  squashRows: S.SQUASH_ROWS,
  legIn: { s: S.LEG_IN, x: 4 },
  legPoses: S.LEG_POSES,
  hindPaws: { s: S.HIND_PAW, x: -2, dy: 1 },
  tails: { rest: S.TAIL, flick: shear(S.TAIL, 0.2) },
  tailAt: ({ tBottom, tail }) => [TX + S.TORSO_W - 3, tBottom - tail.h + 1],
  rightOf: S.rightOf,
});

const far = (s) => recolor(s, { h: 's', b: 's' });

/** Side view, facing right: a low waddle on four stubby legs. */
export function side(o = {}) {
  const lift = o.lift ?? 0;
  const bx = 13;
  const bBottom = 41 - lift;
  const bTop = bBottom - S.SIDE_BODY.h + 1;
  const legY = bBottom - 1;
  const [fl, hl] = o.legs ?? ['down', 'down'];
  const fore = S.SIDE_LEG[fl];
  const hind = S.SIDE_LEG[hl];
  const hy = bTop - 5 + (o.headDy ?? 0);
  const tail = shear(S.SIDE_TAIL, o.tailK ?? 0);
  const groups = [
    group(part(far(fore), bx + 13, legY), part(far(hind), bx + 1, legY)),
    group(part(tail, bx - 10 - tail.ox, bTop - 3)),
    group(part(fore, bx + 11, legY), part(hind, bx + 3, legY)),
    group(part(S.SIDE_BODY, bx, bTop)),
    group(part(S.SIDE_FRILLS, bx + 11, hy - 2)),
    group(
      part(S.SIDE_HEAD, bx + 15, hy),
      part(S.SIDE_EYE, bx + 25, hy + 3),
      part(S.SIDE_SMILE, bx + 26, hy + 6),
      part(S.BLUSH, bx + 23, hy + 6),
    ),
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

function run() {
  return [
    side({ lift: 1, legs: ['fwd', 'back'], tailK: 0.15 }),
    side({ lift: 2, legs: ['tuck', 'tuck'], tailK: 0, headDy: 1 }),
    side({ lift: 1, legs: ['back', 'fwd'], tailK: -0.15 }),
    side({ lift: 0, legs: ['down', 'down'], tailK: 0 }),
  ];
}

const rows = moodRows({
  front,
  run,
  anchors: {
    ask: [34, 3],
    puff: [31, 3],
    cloud: [15, 5],
    drops: [19, 28],
    laptop: () => [15, FLOOR - 10],
    lens: (g) => [g.hx + 18, g.eyeY - 3],
  },
  touch: {
    waiting: (i) => ({ ears: i % 2 ? 'flutter' : 'up' }),
    working: (i) => ({ tail: i % 3 === 1 ? 'flick' : 'rest' }),
  },
});

export default { name: 'axolotl', rows };
