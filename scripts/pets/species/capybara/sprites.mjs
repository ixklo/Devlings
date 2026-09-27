// The capybara (Bean): warm brown, round and calm, with a tall blunt head,
// tiny ears, small sleepy eyes and a broad snout. Wears a little orange on
// its head while idle.
// Roles: o outline, h/b/s/d brown light/base/shadow/deep, w/c/q muzzle and
// belly light/base/shadow, k inner ear, e eyes and nostrils, p blush,
// y/a/r the orange (outlined with O), g/G its leaf.
import { sprite, sym, edgeShade, flipX, recolor, eyePair } from '../../lib/engine.mjs';
import { legSet } from '../../lib/limbs.mjs';

const SHADE = { h: 'b', b: 's', w: 'c', c: 'q' };
const lit = (s) => edgeShade(s, SHADE);
/** A right-hand part from a left-hand one: mirrored, a touch darker. */
export const rightOf = (s) => lit(recolor(flipX(s), { h: 'b' }));

// ---------------------------------------------------------------- head
// A tall rounded block, a little wider at the jowls, with a paler snout.
export const HEAD = lit(
  sym(
    `
    ....hhhhhhh
    ..hhhhbbbbb
    .hhbbbbbbbb
    .hbbbbbbbbb
    hhbbbbbbbbb
    hbbbbbbbbbb
    hbbbbbbbbbb
    hbbbbbbbbbb
    hbbbbbbbbbb
    hbbbbbbbbbb
    hbbbbbbbbbc
    hbbbbbbcccc
    hbbbbbccccc
    hbbbbbccccc
    hbbbbbccccc
    .bbbbbbcccc
    ..bbbbbbbcc
    ....bbbbbbb
  `,
    { right: { h: 'b' } },
  ),
);
export const HEAD_W = HEAD.w; // 22

export const EAR = {
  up: sprite(`
    .hb.
    hkkb
    hkbb
  `),
  droop: sprite(`
    .hbb
    hkkb
    .bb.
  `),
};

// Small, calm eyes.
const EYE_SHAPES = {
  open: `
    ....
    .ee.
    eWee
    .ee.`,
  half: `
    ....
    ....
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
    ....
    eeee
    eWee
    .tt.`,
};
export const EYES = Object.fromEntries(Object.entries(EYE_SHAPES).map(([n, s]) => [n, eyePair(s)]));

export const NOSE = sprite(`
  ee..ee
  ee..ee
  ......
  ..ee..
`);
export const BLUSH = sprite(`
  pp
`);

// The orange: a small citrus with a leaf, balanced on the head.
export const ORANGE = sprite(`
  ..Gg.
  .yya.
  yyaar
  yaaar
  .arr.
`);

// ---------------------------------------------------------------- body
export const TORSO = lit(
  sym(
    `
    .....hbbbbbb
    ...hhbbbbbbb
    ..hbbbbbbbbb
    .hbbbbbbbbbb
    .hbbbbbbbccc
    hbbbbbbbcccc
    hbbbbbbccccc
    hbbbbbbccccc
    hbbbbbbccccc
    hbbbbbbbcccc
    hbbbbbbbbccc
    .bbbbbbbbbbb
    ..bbbbbbbbbb
    ....bbbbbbbb
  `,
    { right: { h: 'b' } },
  ),
);
export const TORSO_W = TORSO.w; // 24
export const SQUASH_ROWS = [4, 7, 2];

const LIMBS = legSet({ paw: 'd' });
export const LEG_IN = LIMBS.legIn;
export const HIND_PAW = LIMBS.hindPaw;
// typing paws tuck under the big head instead of covering the snout
export const LEG_POSES = {
  ...LIMBS.legPoses,
  type: { ...LIMBS.legPoses.type, under: true },
  type2: { ...LIMBS.legPoses.type2, under: true },
};
export const SIDE_LEG = LIMBS.sideLegs;

// ================================================================ SIDE VIEW
export const SIDE_HEAD = lit(
  sprite(`
    ..hb.........
    .hkb.........
    .hhhhhhhh....
    hhbbbbbbbbb..
    hbbbbbbbbbbb.
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbccccc
    bbbbbbbcccccc
    bbbbbbbcccccc
    .bbbbbbbcccc.
    ..bbbbbbbb...
  `),
);
export const SIDE_EYE = sprite(`
  ee
  We
`);
export const SIDE_NOSE = sprite(`
  e
  e
`);
export const SIDE_BODY = lit(
  sprite(`
    ....hhhhhhhhhhh....
    ..hhbbbbbbbbbbbbb..
    .hbbbbbbbbbbbbbbbb.
    hbbbbbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbbbbcc
    bbbbbbbbbbbbbbbbccc
    bbbbbbbbbbbbbbbbccc
    .bbbbbbbbbbbbbbbbc.
    ..bbbbbbbbbbbbbbb..
    ....bbbbbbbbbbb....
  `),
);
