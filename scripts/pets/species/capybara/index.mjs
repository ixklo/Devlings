// Species "capybara" (Bean). Personality: unbothered; a tiny orange sits on
// its head while it idles.
import { compose, dropRows, recolor } from '../../lib/engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from '../../lib/palettes.mjs';
import { moodRows, part, group } from '../../lib/moods.mjs';
import { makeSitter } from '../../lib/sitter.mjs';
import * as S from './sprites.mjs';

const FLOOR = 45;
const TX = 12;
const HX = 13;

export const front = makeSitter({
  floor: FLOOR,
  head: S.HEAD,
  headX: HX,
  overlap: 4,
  ears: { up: { s: S.EAR.up, x: 1, y: -2 }, droop: { s: S.EAR.droop, x: -1, y: 0 } },
  eyes: S.EYES,
  eyeAt: { lx: 2, rx: 16, y: 4 },
  face: [
    { s: S.BLUSH, x: 1, y: 8 },
    { s: S.BLUSH, x: 19, y: 8 },
    { s: S.NOSE, x: 8, y: 11 },
  ],
  torso: S.TORSO,
  torsoX: TX,
  squashRows: S.SQUASH_ROWS,
  legIn: { s: S.LEG_IN, x: 5 },
  legPoses: S.LEG_POSES,
  hindPaws: { s: S.HIND_PAW, x: -1, dy: 1 },
  tails: { rest: null },
  tailAt: () => [0, 0],
  rightOf: S.rightOf,
  // the orange rides on the head, so it squashes and leans with it
  extras: (o, g) => (o.orange ? { head: [part(S.ORANGE, g.hx + 9, g.top - 4)] } : {}),
});

const far = (s) => recolor(s, { h: 's', b: 's', d: 'o' });

/** Side view, facing right: a round loaf trotting on stubby legs. */
export function side(o = {}) {
  const lift = o.lift ?? 0;
  const body = o.squash ? dropRows(S.SIDE_BODY, [5]) : S.SIDE_BODY;
  const bx = 8;
  const bBottom = 41 - lift;
  const bTop = bBottom - body.h + 1;
  const legY = bBottom - 1;
  const [fl, hl] = o.legs ?? ['down', 'down'];
  const fore = S.SIDE_LEG[fl];
  const hind = S.SIDE_LEG[hl];
  const hy = bTop - 6 + (o.headDy ?? 0);
  const groups = [
    group(part(far(fore), bx + 14, legY), part(far(hind), bx + 2, legY)),
    group(part(fore, bx + 12, legY), part(hind, bx + 4, legY)),
    group(part(body, bx, bTop)),
    group(
      part(S.SIDE_HEAD, bx + 17, hy),
      part(S.SIDE_EYE, bx + 23, hy + 4),
      part(S.SIDE_NOSE, bx + 29, hy + 7),
      part(S.BLUSH, bx + 22, hy + 8),
    ),
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

function run() {
  return [
    side({ lift: 1, legs: ['fwd', 'back'] }),
    side({ lift: 2, legs: ['tuck', 'tuck'], headDy: 1 }),
    side({ lift: 1, legs: ['back', 'fwd'] }),
    side({ lift: 0, legs: ['down', 'down'], squash: 1 }),
  ];
}

const rows = moodRows({
  front,
  run,
  anchors: {
    ask: [34, 3],
    puff: [31, 3],
    cloud: [15, 4],
    drops: [19, 28],
    laptop: () => [15, FLOOR - 10],
    lens: (g) => [g.hx + 12, g.eyeY - 3],
  },
  touch: {
    idle: () => ({ orange: true }),
  },
});

export default { name: 'capybara', rows };
