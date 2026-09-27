// Species "cat" (Miso). Personality: smug, heavy-lidded looks, and a big
// stretch (lens held high) at the end of every review.
import { compose, dropRows, recolor } from '../../lib/engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from '../../lib/palettes.mjs';
import * as P from '../../lib/props.mjs';
import { moodRows, part, group } from '../../lib/moods.mjs';
import { makeSitter } from '../../lib/sitter.mjs';
import * as S from './sprites.mjs';

const FLOOR = 45;
const TX = 13;
const HX = 10;

export const front = makeSitter({
  floor: FLOOR,
  head: S.HEAD,
  headX: HX,
  overlap: 3,
  ears: { up: { s: S.EAR.up, x: 1, y: -4 }, droop: { s: S.EAR.droop, x: -2, y: 0 } },
  eyes: S.EYES,
  eyeAt: { lx: 5, rx: 18, y: 6 },
  face: [
    { s: S.BLUSH, x: 3, y: 11 },
    { s: S.BLUSH, x: 23, y: 11 },
    { s: S.NOSE, x: 12, y: 11 },
  ],
  torso: S.TORSO,
  torsoX: TX,
  squashRows: S.SQUASH_ROWS,
  legIn: { s: S.LEG_IN, x: 5 },
  legPoses: S.LEG_POSES,
  hindPaws: { s: S.HIND_PAW, x: -1, dy: 1 },
  tails: { rest: S.TAIL, down: S.TAIL_DOWN },
  tailAt: ({ tBottom, tail }) => (tail === S.TAIL_DOWN ? [TX + S.TORSO_W - 3, tBottom - 1] : [TX + S.TORSO_W - 1, tBottom - tail.h - 1]),
  rightOf: S.rightOf,
  // whiskers cross the cheek outline, so they get none of their own
  extras: (o, g) => ({
    front: [
      {
        parts: [part(S.WHISKERS, g.hx - 3, g.top + 10), part(S.rightOf(S.WHISKERS), g.hx + S.HEAD_W - 1, g.top + 10)],
        outline: false,
      },
    ],
  }),
});

const far = (s) => recolor(s, { h: 's', b: 's', w: 'q' });

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
  const hy = bTop - 8 + (o.headDy ?? 0);
  const groups = [
    group(part(far(fore), bx + 12, legY), part(far(hind), bx + 1, legY)),
    group(part(S.SIDE_TAIL, bx - 4, bTop - 7 + (o.tailDy ?? 0))),
    group(part(fore, bx + 10, legY), part(hind, bx + 3, legY)),
    group(part(body, bx, bTop)),
    group(part(S.SIDE_HEAD, bx + 13, hy), part(S.SIDE_EYE, bx + 22, hy + 5), part(S.BLUSH, bx + 20, hy + 9)),
    {
      parts: [part(S.WHISKERS, bx + 25, hy + 8)],
      outline: false,
    },
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

function run() {
  return [
    side({ lift: 2, legs: ['fwd', 'back'], tailDy: 0 }),
    side({ lift: 4, legs: ['tuck', 'tuck'], tailDy: -1, headDy: 1 }),
    side({ lift: 1, legs: ['back', 'fwd'], tailDy: 0 }),
    side({ lift: 0, legs: ['down', 'down'], squash: 1, tailDy: 1 }),
  ];
}

// The stretch: both paws up, the lens held high in the right one.
const stretch = () => ({
  squash: -2,
  armL: 'stretch',
  armR: 'stretch',
  eyes: 'happy',
  props: (g) => {
    const lx = TX + S.TORSO_W - 5;
    const ly = g.tTop - 25;
    return { front: [group(part(P.HANDLE, lx + 8, ly + 8)), group(part(P.LENS, lx, ly))] };
  },
});

const rows = moodRows({
  front,
  run,
  bigEye: S.BIG_EYE,
  anchors: {
    ask: [34, 3],
    puff: [31, 3],
    cloud: [15, 3],
    drops: [19, 28],
    laptop: () => [15, FLOOR - 10],
    lens: (g) => [g.hx + 15, g.eyeY - 3],
  },
  touch: {
    failed: () => ({ tail: 'down' }),
    review: (i) => (i === 5 ? stretch() : { eyes: 'half' }),
  },
});

export default { name: 'cat', rows };
