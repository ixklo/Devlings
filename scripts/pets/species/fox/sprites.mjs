// The fox (Pip): russet with a white chest, muzzle and tail tip, dark socks
// and ear tips. Sits upright facing you; runs on all fours.
// Roles: o outline, h/b/s/d russet light/base/shadow/deep, w/c/q white
// light/base/shadow, k dark socks and ear tips, e eyes and nose, p blush.
import { sprite, sym, edgeShade, flipX, recolor, eyePair } from '../../lib/engine.mjs';
import { legSet } from '../../lib/limbs.mjs';

const SHADE = { h: 'b', b: 's', w: 'c', c: 'q' };
const lit = (s) => edgeShade(s, SHADE);
/** A right-hand part from a left-hand one: mirrored, a touch darker. */
export const rightOf = (s) => lit(recolor(flipX(s), { h: 'b' }));

// ---------------------------------------------------------------- head
export const HEAD = lit(
  sym(
    `
    ......hhhhhhhh
    ....hhhhbbbbbb
    ...hhbbbbbbbbb
    ..hbbbbbbbbbbb
    .hbbbbbbbbbbbb
    .hbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    wwbbbbbbbbbbbb
    wwwwbbbbbbbbbb
    wwwwwwwbbbbbbb
    .wwwwwwwwwwbbb
    ..wwwwwwwwwwww
    ...wwwwwwwwwww
    .....wwwwwwwww
  `,
    { right: { h: 'b' } },
  ),
);
export const HEAD_W = HEAD.w; // 28

export const EAR = {
  up: sprite(`
    .k.....
    .kk....
    hkkb...
    hbcbb..
    hbccbb.
    hbcccbb
    hbcccbb
  `),
  // folded down and out (sad): the tip points away and down
  droop: sprite(`
    .....hb
    ...hbcb
    .hbccbb
    kkbccb.
    kkkbb..
    .k.....
  `),
};

const EYE_SHAPES = {
  open: `
    .ee.
    eWee
    eeee
    eeee
    .ee.`,
  half: `
    ....
    ....
    eeee
    eWee
    .ee.`,
  blink: `
    ....
    ....
    ....
    e..e
    .ee.`,
  happy: `
    ....
    .ee.
    e..e
    ....
    ....`,
  up: `
    .ee.
    eWee
    eeee
    eeWe
    .ee.`,
  teary: `
    .ee.
    eWee
    eeee
    ette
    .tt.`,
};
export const EYES = Object.fromEntries(Object.entries(EYE_SHAPES).map(([n, s]) => [n, eyePair(s)]));

export const NOSE = sprite(`
  .ee.
  e..e
`);
export const NOSE_TIP = sprite(`
  ee
`);
export const BLUSH = sprite(`
  pp
`);

// ---------------------------------------------------------------- body
export const TORSO = lit(
  sym(`
    ....hbbbwww
    ...hbbbbwww
    ..hbbbbbwww
    ..hbbbbbwww
    .hbbbbbbwww
    .hbbbbbbwww
    .hbbbbbbwww
    hbbbbbbbwww
    hbbbbbbbwww
    hbbbbbbbbww
    hbbbbbbbbbb
    bbbbbbbbbbb
    .bbbbbbbbbb
    ..bbbbbbbbb
  `, { right: { h: 'b' } }),
);
export const TORSO_W = TORSO.w; // 22
export const SQUASH_ROWS = [4, 7, 2];

// Front legs and hind paws: the shared sitting-pet limbs with dark socks.
const LIMBS = legSet({ paw: 'k' });
export const LEG_IN = LIMBS.legIn;
export const HIND_PAW = LIMBS.hindPaw;
export const LEG_POSES = LIMBS.legPoses;

// ---------------------------------------------------------------- tail
// Rises from behind the right haunch; the white tip is the brightest spot.
export const TAIL = lit(
  sprite(`
    ........www..
    .......wwwww.
    ......hwwwwc.
    .....hbbwwcc.
    ....hbbbbcc..
    ...hbbbbbs...
    ..hbbbbbs....
    .hbbbbbs.....
    hbbbbbs......
    hbbbbs.......
    hbbbbs.......
    bbbbs........
    .bbs.........
  `),
);

// ================================================================ SIDE VIEW
// Facing right (running-right); running-left mirrors whole frames.
export const SIDE_HEAD = lit(
  sprite(`
    ....k...k.......
    ...kb..kb.......
    ...hbb.hbb......
    ..hbcb.hcbb.....
    ..hbbbhbbbbb....
    .hbbbbbbbbbbbb..
    hbbbbbbbbbbbbbb.
    hbbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbbe
    bbbbbbbbbbwwwwww
    wwbbbbbbwwwwwww.
    .wwwwwbwwwwwww..
    ..wwwwwwwwwww...
    ....wwwwwwww....
  `),
);
export const SIDE_EYE = {
  open: sprite(`
    ee
    We
    ee
  `),
  happy: sprite(`
    ..
    ee
    ..
  `),
};

export const SIDE_BODY = lit(
  sprite(`
    ....hhhhhhhhhh....
    ..hhbbbbbbbbbbbb..
    .hbbbbbbbbbbbbbbw.
    hbbbbbbbbbbbbbbwww
    hbbbbbbbbbbbbbwwww
    bbbbbbbbbbbbbbwwww
    bbbbbbbbbbbbbbbwww
    .bbbbbbbbbbbbbbbw.
    ..bbbbbbbbbbbbbb..
    ....bbbbbbbbbb....
  `),
);
export const SIDE_TAIL = lit(
  sprite(`
    www..........
    wwwwhh.......
    .wwwbbbbh....
    ..ccbbbbbbbh.
    ....bbbbbbbbb
    ......bbbbbbb
    .........bbbb
  `),
);
export const SIDE_LEG = LIMBS.sideLegs;
