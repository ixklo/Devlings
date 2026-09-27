// Species "robot" (Bolt). Personality: the antenna light blinks while it works.
import { compose } from '../../lib/engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from '../../lib/palettes.mjs';
import { moodRows, part, group } from '../../lib/moods.mjs';
import { makeSitter } from '../../lib/sitter.mjs';
import * as S from './sprites.mjs';

const FLOOR = 45;
const TX = 14;
const HX = 11;

export const front = makeSitter({
  floor: FLOOR - 1,
  head: S.HEAD,
  headX: HX,
  overlap: 2,
  ears: { up: { s: S.BOLT, x: -2, y: 6 }, droop: { s: S.BOLT, x: -2, y: 6 } },
  eyes: S.EYES,
  eyeAt: { lx: 6, rx: 16, y: 5 },
  face: [
    { s: S.BLUSH, x: 3, y: 11 },
    { s: S.BLUSH, x: 21, y: 11 },
    { s: S.MOUTH, x: 10, y: 11 },
  ],
  torso: S.TORSO,
  torsoX: TX,
  squashRows: S.SQUASH_ROWS,
  legIn: null,
  legPoses: S.LEG_POSES,
  hindPaws: { s: S.FOOT, x: 3, dy: 0 },
  tails: { rest: null },
  tailAt: () => [0, 0],
  rightOf: S.rightOf,
  extras: (o, g) => {
    const antenna = o.droop ? S.ANTENNA_DROOP : S.ANTENNA[o.light ?? 'on'];
    return {
      head: [part(antenna, g.hx + 11 - (o.droop ? 2 : 0), g.top - antenna.h + 1)],
      front: [{ parts: [part(S.PANEL_LIGHTS, TX + 8, g.tTop + 4)], outline: false }],
    };
  },
});

/** Side view, facing right: a brisk walk, arms swinging. */
export function side(o = {}) {
  const lift = o.lift ?? 0;
  const bx = 16;
  const bBottom = 39 - lift;
  const torso = S.SIDE_TORSO;
  const bTop = bBottom - torso.h + 1;
  const hy = bTop - 13 + (o.headDy ?? 0);
  const [near, far] = o.legs ?? ['down', 'down'];
  const legN = S.SIDE_LEGS[near];
  const legF = S.SIDE_LEGS[far];
  const arm = S.SIDE_ARM[o.arm ?? 'down'];
  const antenna = S.ANTENNA.on;
  const groups = [
    group(part(legF, bx + 8 - (legF.ox ?? 0), bBottom)),
    group(part(legN, bx + 3 - (legN.ox ?? 0), bBottom)),
    group(part(torso, bx, bTop)),
    group(
      part(antenna, bx + 6, hy - antenna.h + 1),
      part(S.SIDE_HEAD, bx - 3, hy),
      part(S.SIDE_BOLT, bx + 2, hy + 6),
      part(S.SIDE_EYE, bx + 13, hy + 5),
      part(S.BLUSH, bx + 11, hy + 11),
    ),
    group(part(arm, bx + 5 - (arm.ox ?? 0), bTop + 1)),
  ];
  return compose(groups, OUTLINE_OF, LINE_ROLES);
}

function run() {
  return [
    side({ lift: 1, legs: ['fwd', 'back'], arm: 'back' }),
    side({ lift: 2, legs: ['down', 'down'], arm: 'down' }),
    side({ lift: 1, legs: ['back', 'fwd'], arm: 'fwd' }),
    side({ lift: 0, legs: ['down', 'down'], arm: 'down', headDy: 1 }),
  ];
}

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
    lens: (g) => [g.hx + 13, g.eyeY - 3],
  },
  touch: {
    working: (i) => ({ light: i % 2 ? 'off' : 'on' }),
  },
});

export default { name: 'robot', rows };
