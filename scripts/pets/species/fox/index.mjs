// Species "fox" (Pip). Personality: the tail swishes while it works.
import { compose, dropRows, shear, recolor } from '../../lib/engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from '../../lib/palettes.mjs';
import { moodRows, part, group } from '../../lib/moods.mjs';
import { makeSitter } from '../../lib/sitter.mjs';
import * as S from './sprites.mjs';

const FLOOR = 45; // lowest fill row; outlines land on 46, level with the bird's branch
const TX = 13; // torso left

export const front = makeSitter({
  floor: FLOOR,
  head: S.HEAD,
  headX: 10,
  overlap: 3,
  ears: { up: { s: S.EAR.up, x: 1, y: -5 }, droop: { s: S.EAR.droop, x: -4, y: 0 } },
  eyes: S.EYES,
  eyeAt: { lx: 6, rx: 18, y: 7 },
  face: [
    { s: S.BLUSH, x: 3, y: 12 },
    { s: S.BLUSH, x: 23, y: 12 },
    { s: S.NOSE, x: 12, y: 13 },
  ],
  torso: S.TORSO,
  torsoX: TX,
  squashRows: S.SQUASH_ROWS,
  legIn: { s: S.LEG_IN, x: 5 },
  legPoses: S.LEG_POSES,
  hindPaws: { s: S.HIND_PAW, x: -1, dy: 1 },
  tails: { rest: S.TAIL, left: shear(S.TAIL, -0.25), right: shear(S.TAIL, 0.2) },
  tailAt: ({ tBottom, tail }) => [TX + S.TORSO_W - 3, tBottom - tail.h + 1],
  rightOf: S.rightOf,
});

const far = (s) => recolor(s, { h: 's', b: 's', k: 'd' });

/** Side view, facing right, on all fours. */
export function side(o = {}) {
  const lift = o.lift ?? 0;
  const body = o.squash ? dropRows(S.SIDE_BODY, [4]) : S.SIDE_BODY;
  const bx = 10;
  const bBottom = 40 - lift;
  const bTop = bBottom - body.h + 1;
  const legY = bBottom - 1;
  const [fl, hl] = o.legs ?? ['down', 'down'];
  const fore = S.SIDE_LEG[fl];
  const hind = S.SIDE_LEG[hl];
  const hy = bTop - 9 + (o.headDy ?? 0);
  const groups = [
    group(part(far(fore), bx + 13, legY), part(far(hind), bx + 1, legY)),
    group(part(S.SIDE_TAIL, bx - 8, bTop - 2 + (o.tailDy ?? 0))),
    group(part(fore, bx + 11, legY), part(hind, bx + 3, legY)),
    group(part(body, bx, bTop)),
    group(
      part(S.SIDE_HEAD, bx + 14, hy),
      part(S.SIDE_EYE[o.eyes ?? 'open'], bx + 25, hy + 5),
      part(S.BLUSH, bx + 23, hy + 9),
    ),
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

function run() {
  return [
    side({ lift: 2, legs: ['fwd', 'back'], tailDy: -1 }),
    side({ lift: 4, legs: ['tuck', 'tuck'], tailDy: -2, headDy: 1 }),
    side({ lift: 1, legs: ['back', 'fwd'], tailDy: 0 }),
    side({ lift: 0, legs: ['down', 'down'], squash: 1, tailDy: 1 }),
  ];
}

const rows = moodRows({
  front,
  run,
  anchors: {
    ask: [34, 3],
    puff: [31, 3],
    cloud: [15, 2],
    drops: [19, 28],
    laptop: () => [15, FLOOR - 10],
    lens: (g) => [g.hx + 15, g.eyeY - 2],
  },
  touch: {
    working: (i) => ({ tail: ['rest', 'right', 'right', 'rest', 'left', 'left'][i] }),
  },
});

export default { name: 'fox', rows };
