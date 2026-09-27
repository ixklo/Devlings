// The axolotl (Nori): pink, with a broad flat head, beady eyes, a wide smile,
// three feathery gill frills on each side and a finned tail.
// Roles: o outline, h/b/s/d pink light/base/shadow/deep, w/c/q belly
// light/base/shadow, k/j frill base/tip, e eyes and smile, p blush.
import { sprite, sym, edgeShade, flipX, recolor, eyePair } from '../../lib/engine.mjs';
import { legSet } from '../../lib/limbs.mjs';

const SHADE = { h: 'b', b: 's', w: 'c', c: 'q' };
const lit = (s) => edgeShade(s, SHADE);
/** A right-hand part from a left-hand one: mirrored, a touch darker. */
export const rightOf = (s) => lit(recolor(flipX(s), { h: 'b', j: 'k' }));

// ---------------------------------------------------------------- head
export const HEAD = lit(
  sym(
    `
    .....hhhhhhhhhh
    ...hhhhhbbbbbbb
    ..hhbbbbbbbbbbb
    .hhbbbbbbbbbbbb
    .hbbbbbbbbbbbbb
    hhbbbbbbbbbbbbb
    hbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbb
    .bbbbbbbbbbbbbb
    ..bbbbbbbbbbbbb
    ....bbbbbbbbbbb
  `,
    { right: { h: 'b' } },
  ),
);
export const HEAD_W = HEAD.w; // 30

// Gill frills, left side: three feathery stalks fanning out from the head.
export const FRILLS = {
  up: sprite(`
    jkk.....
    .jkkk...
    ..jkkkk.
    .....kkk
    jjkkkkkk
    .....kkk
    ..jkkkk.
    .jkkk...
    jkk.....
  `),
  // spread wider: the flutter (waiting)
  flutter: sprite(`
    jk......
    .jkk....
    ..jkk...
    ...jkkk.
    .....kkk
    jjkkkkkk
    .....kkk
    ...jkkk.
    ..jkk...
    .jkk....
    jk......
  `),
  // hanging limp (failed)
  droop: sprite(`
    ......kk
    ....kkkk
    ..jkkkkk
    .jkk.kkk
    jkk.jkkk
    jk..jkk.
    ...jkk..
    ...jk...
  `),
};

const EYE_SHAPES = {
  open: `
    .ee.
    eWee
    eeee
    .ee.`,
  half: `
    ....
    eeee
    eeee
    .ee.`,
  blink: `
    ....
    ....
    e..e
    .ee.`,
  happy: `
    ....
    .ee.
    e..e
    ....`,
  up: `
    .ee.
    eWee
    eeWe
    .ee.`,
  teary: `
    .ee.
    eWee
    ette
    .tt.`,
};
export const EYES = Object.fromEntries(Object.entries(EYE_SHAPES).map(([n, s]) => [n, eyePair(s)]));

// The wide axolotl smile, turned up at the corners.
export const SMILE = sprite(`
  e........e
  .eeeeeeee.
`);
export const BLUSH = sprite(`
  pp
`);

// ---------------------------------------------------------------- body
export const TORSO = lit(
  sym(
    `
    ...hbbbbbw
    ..hbbbbbww
    ..hbbbbwww
    .hbbbbwwww
    .hbbbbwwww
    .hbbbbwwww
    hbbbbbwwww
    hbbbbbbwww
    hbbbbbbbww
    bbbbbbbbbb
    .bbbbbbbbb
    ..bbbbbbbb
  `,
    { right: { h: 'b' } },
  ),
);
export const TORSO_W = TORSO.w; // 20
export const SQUASH_ROWS = [4, 7, 2];

const LIMBS = legSet({ paw: 'h' });
export const LEG_IN = LIMBS.legIn;
export const HIND_PAW = LIMBS.hindPaw;
// the raised paw goes higher than the other sitters': the head is lower and wider
export const LEG_POSES = { ...LIMBS.legPoses, up: { ...LIMBS.legPoses.up, x: -6, y: -12 } };
export const SIDE_LEG = LIMBS.sideLegs;

// ---------------------------------------------------------------- tail
// Broad and flat with a pale fin along its edge, curling up at the tip.
export const TAIL = lit(
  sprite(`
    .........jj.
    .......jjbbj
    .....jjbbbb.
    ...jjbbbbs..
    .jjbbbbbs...
    jbbbbbss....
    hbbbbss.....
    bbbss.......
    .ss.........
  `),
);

// ================================================================ SIDE VIEW
export const SIDE_HEAD = lit(
  sprite(`
    ....hhhhhhhh....
    ..hhhbbbbbbbbb..
    .hhbbbbbbbbbbbb.
    hhbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbbb
    .bbbbbbbbbbbbbb.
    ..bbbbbbbbbbbb..
    ....bbbbbbbb....
  `),
);
export const SIDE_FRILLS = sprite(`
  jkk....
  .jkkk..
  ...kkkk
  jjkkkkk
  ...kkkk
  .jkkk..
  jkk....
`);
export const SIDE_EYE = sprite(`
  ee
  We
`);
export const SIDE_SMILE = sprite(`
  eeee.
  ....e
`);
export const SIDE_BODY = lit(
  sprite(`
    ...hhhhhhhhhhh...
    .hhbbbbbbbbbbbbb.
    hbbbbbbbbbbbbbbbb
    bbbbbbbbbbbbbbbbw
    bbbbbbbbbbbbbbwww
    .bbbbbbbbbbbwwww.
    ..bbbbbbbbbbbbb..
  `),
);
export const SIDE_TAIL = lit(
  sprite(`
    jj..........
    bbjj........
    .bbbjjj.....
    ..bbbbbbjj..
    ...sbbbbbbbb
    .....ssbbbbb
    ........ssss
  `),
);
