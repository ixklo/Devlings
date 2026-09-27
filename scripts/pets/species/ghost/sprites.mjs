// The ghost (Wisp): soft white with a pale lavender outline, a curl at the
// crown, a scalloped hem, little nub arms, big dark eyes. It never touches
// the ground.
// Roles: o outline, c faded outline, h/b/s/d white light/base, lavender
// shadow/deep, e eyes and mouth, p blush.
import { sprite, sym, edgeShade, flipX, recolor, eyePair } from '../../lib/engine.mjs';

const SHADE = { h: 'b', b: 's' };
const lit = (s) => edgeShade(s, SHADE);
/** A right-hand part from a left-hand one: mirrored, a touch darker. */
export const rightOf = (s) => lit(recolor(flipX(s), { h: 'b' }));

// ---------------------------------------------------------------- body
export const BODY = lit(
  sym(
    `
    ........hhhhh
    ......hhhhhhh
    .....hhhhbbbb
    ....hhhbbbbbb
    ...hhbbbbbbbb
    ..hhbbbbbbbbb
    ..hbbbbbbbbbb
    .hhbbbbbbbbbb
    .hbbbbbbbbbbb
    .hbbbbbbbbbbb
    hhbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    bbbbbbbbbbbbb
    bbbbbbbbbbbbb
    .bbbbbb.bbbbb
    ..bbbb...bbbb
    ...bb.....bbb
    ...........bb
  `,
    { right: { h: 'b' } },
  ),
);
export const BODY_W = BODY.w; // 26
export const SQUASH_ROWS = [17, 14, 19];

// A little curl at the crown, the "wisp".
export const CURL = {
  up: sprite(`
    hh..
    .hb.
    .hbb
  `),
  droop: sprite(`
    ....
    hhb.
    .hbb
  `),
};

const EYE_SHAPES = {
  open: `
    .ee.
    eWWe
    eWee
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
    eWWe
    eWee
    eeWe
    .ee.`,
  teary: `
    .ee.
    eWWe
    eWee
    ette
    .tt.`,
};
export const EYES = Object.fromEntries(Object.entries(EYE_SHAPES).map(([n, s]) => [n, eyePair(s)]));

export const MOUTH = sprite(`
  e..e
  .ee.
`);
export const BLUSH = sprite(`
  pp
`);

// Nub arms (left); x, y from the body's top-left.
const ARM_REST = sprite(`
  ..hb
  .hbb
  hbbs
  hbs.
  .s..
`);
const ARM_FLAT = sprite(`
  ..hhbb
  hhbbbs
  .sss..
`);
const ARM_DIAG = sprite(`
  hb....
  hbb...
  .bbbs.
  ..bsss
`);
const ARM_UP = sprite(`
  .h.
  hbb
  hbb
  hbs
  .bs
`);
const ARM_TYPE = sprite(`
  .hb
  hbb
  hbb
  bbs
`);
const ARM_TYPE2 = sprite(`
  .hb
  hbb
  bbs
`);
const ARM_HOLD = sprite(`
  hb...
  hbbb.
  .bbbs
  ..ss.
`);
export const ARM_POSES = {
  rest: { s: ARM_REST, x: -3, y: 15 },
  droop: { s: ARM_REST, x: -2, y: 17 },
  flat: { s: ARM_FLAT, x: -5, y: 14 },
  diag: { s: ARM_DIAG, x: -5, y: 10 },
  up: { s: ARM_UP, x: -3, y: 8 },
  type: { s: ARM_TYPE, x: 4, y: 16 },
  type2: { s: ARM_TYPE2, x: 4, y: 15 },
  hold: { s: ARM_HOLD, x: 0, y: 14 },
};

// ================================================================ SIDE VIEW
// Drifting right, the hem streaming back into a curled tail.
export const SIDE_BODY = lit(
  sprite(`
    ..............hhhhhh......
    ...........hhhhhhbbbbb....
    .........hhhhbbbbbbbbbb...
    ........hhbbbbbbbbbbbbbb..
    .......hhbbbbbbbbbbbbbbbb.
    .......hbbbbbbbbbbbbbbbbb.
    ......hhbbbbbbbbbbbbbbbbbb
    ......hbbbbbbbbbbbbbbbbbbb
    ......hbbbbbbbbbbbbbbbbbbb
    ......hbbbbbbbbbbbbbbbbbbb
    ......hbbbbbbbbbbbbbbbbbbb
    ......bbbbbbbbbbbbbbbbbbbb
    ......bbbbbbbbbbbbbbbbbbbb
    .....bbbbbbbbbbbbbbbbbbbbb
    .....bbbbbbbbbbbbbbbbbbbb.
    ....bbbbbbbbbbbbbbbbbbbbb.
    ...bbbbbbbbbbbbbbbbbbbbb..
    ..bbbbbbbbbbbbbbbbbbbbb...
    .bbbbbbbbbbbbbbbbbbbbb....
    hbbbbbbbbbbbbbbbbbbbb.....
    hbbbb..bbbbbbbbbbbb.......
    hbb.....bbbbbb.bbb........
    .hb......bbb....b.........
  `),
);
export const SIDE_ARM = sprite(`
  .hhb
  hbbbs
  .sss.
`);
