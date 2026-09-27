// The bird (Perch, Ember, Plum): hand-authored pixel maps. One character =
// one art pixel (4x4 screen px). Roles are defined in lib/palettes.mjs. Maps
// hold fill colours; outlines are added automatically per group
// (engine.outline). Line roles such as 'e', '9' or '8' are used for interior
// detail and never grow an outline.
import { sprite, flipX, flipY, recolor, eyePair } from '../../lib/engine.mjs';

// ================================================================ FRONT VIEW
// One round head-body blob with a cream belly. Light from the top-left:
// highlight cap on the crown, shadow crescent bottom-right.
export const BODY = sprite(`
  ..........bbbbbbbbbb..........
  ........bbhhhhbbbbbbbb........
  ......bbhhhhhhhbbbbbbbbb......
  .....bbhhhhhbbbbbbbbbbbbb.....
  ....bbhhhbbbbbbbbbbbbbbbbs....
  ...bbhhbbbbbbbbbbbbbbbbbbss...
  ..bbbhbbbbbbbbbbbbbbbbbbbbss..
  ..bbhhbbbbbbbbbbbbbbbbbbbbss..
  .bbbhbbbbbbbbbbbbbbbbbbbbbbss.
  .bbbhbbbbbbbbbbbbbbbbbbbbbbss.
  .bbbbbbbbbbbbbbbbbbbbbbbbbbss.
  bbbbbbbbbbbbbbbbbbbbbbbbbbbsss
  bbbbbbbbbbbbbbbbbbbbbbbbbbbsss
  bbbbbbbbbbbwwwwwwcqbbbbbbbbsss
  bbbbbbbbbwwwwcccccccqbbbbbbsss
  bbbbbbbbwwwccccccccccqbbbbbsss
  bbbbbbbwwccccccccccccqqbbbbsss
  bbbbbbwwccccccccccccccqqbbssss
  bbbbbbwcccccccccccccccqqbbssss
  bbbbbwwccccccccccccccccqqbssss
  bbbbbwcccccccccccccccccqqbssss
  .sbbbwcccccccccccccccccqqssss.
  .ssbbwccccccccccccccccqqqssss.
  ..ssbccccccccccccccccqqqqsss..
  ...ssbccccccccccccccqqqqsss...
  ....ssqccccccccccccqqqqqss....
  ......ssqqcccccccqqqqqss......
  .........sqqqqqqqqqqs.........
`);
export const BODY_W = BODY.w; // 30
export const BODY_H = BODY.h; // 28
// Rows below the face that can be removed/duplicated for squash & stretch.
export const SQUASH_ROWS = [17, 20, 15];

// Eyes are 5x6. 'W' is the specular highlight (top-left, matching the light).
const EYE_SHAPES = {
  open: `
    .eee.
    eWWee
    eWWee
    eeeee
    eeeee
    .eee.`,
  half: `
    .....
    .....
    .eee.
    eWeee
    eeeee
    .eee.`,
  blink: `
    .....
    .....
    .....
    e...e
    .eee.
    .....`,
  happy: `
    .....
    .eee.
    e...e
    .....
    .....
    .....`,
  // extra sparkle: expectant, looking up at you
  up: `
    .eee.
    eWWee
    eWWee
    eeeee
    eeeWe
    .eee.`,
  // welling up: tears pool along the lower lid
  teary: `
    .eee.
    eWWee
    eWWee
    eeeee
    ettte
    .ttt.`,
};
/** Each eye comes as a left/right pair: shapes mirror, highlights stay top-left. */
export const EYES = Object.fromEntries(Object.entries(EYE_SHAPES).map(([name, src]) => [name, eyePair(src)]));

export const BEAK = sprite(`
  yyaa
  .ar.
`);

export const BLUSH = sprite(`
  pp
`);

export const TUFT = {
  // a single curled feather, echoing the app icon
  up: sprite(`
    ...hhb..
    ..hb.bb.
    ..b...b.
    ......b.
    ....bbb.
    ...bbb..
    ...bb...
  `),
  sway: sprite(`
    ....hhb.
    ...hb.bb
    ...b...b
    .......b
    ....bbbb
    ...bbb..
    ...bb...
  `),
  droop: sprite(`
    ........
    ........
    .hhhbb..
    hb..bbb.
    b...bb..
    b..bbb..
    ...bb...
  `),
};

// ---------------------------------------------------------------- wings
// Left wing (viewer's left) poses; x,y are offsets from the body's top-left.
const WING_REST = sprite(`
  ..hb
  .hbb
  hbbs
  hbbs
  bbss
  bbss
  bsss
  .sss
  ..s.
`);
const WING_DIAG = sprite(`
  hb....
  hbb...
  .bbbb.
  ..bbss
  ...sss
  ....ss
`);
const WING_FLAT = sprite(`
  .hhbb..
  hbbbbss
  .sssss.
`);
const WING_UP = flipY(WING_REST);
const WING_TYPE = sprite(`
  ...hb
  ..hbb
  .hbbs
  hbbss
  bbss.
  sss..
`);
const WING_TYPE2 = sprite(`
  ...hb
  ..hbb
  .hbbs
  .bbss
  .bss.
  .ss..
`);
const WING_DROOP = sprite(`
  ..hb
  .hbb
  .bbs
  hbbs
  bbss
  bsss
  bsss
  bsss
  .ss.
  .s..
`);
const WING_HOLD = sprite(`
  ...hbb
  ..hbbs
  .hbbss
  hbbss.
  bsss..
`);

export const WING_POSES = {
  rest: { s: WING_REST, x: -1, y: 14 },
  diag: { s: WING_DIAG, x: -5, y: 9 },
  flat: { s: WING_FLAT, x: -6, y: 13 },
  up: { s: WING_UP, x: -3, y: 5 },
  type: { s: WING_TYPE, x: 2, y: 15 },
  type2: { s: WING_TYPE2, x: 2, y: 11 },
  droop: { s: WING_DROOP, x: -1, y: 15 },
  hold: { s: WING_HOLD, x: 0, y: 15 },
};

/** Right wing pose: mirror of the left one, a touch darker (away from the light). */
export function rightWing(pose) {
  const p = WING_POSES[pose];
  const s = recolor(flipX(p.s), { h: 'b' });
  return { s, x: BODY_W - p.x - p.s.w, y: p.y };
}

// ---------------------------------------------------------------- tail, feet
export const TAIL = sprite(`
  ....bs..
  ..bbss..
  .bbsssds
  bssddss.
  .sdd....
`);

export const FOOT = sprite(`
  .a.
  aaa
  a.a
`);

// ---------------------------------------------------------------- branch
export const BRANCH = sprite(`
  .gg..................................
  gGG..................................
  .G2..................................
  ..2111111111111111111111111111111111.
  ..12222223222222222222222232222222231
  ...33333333333333333333333333333333..
`);
export const BRANCH_TOP = 3; // row of the branch's top surface inside the map

// ================================================================ SIDE VIEW
// Facing right (running-right). running-left mirrors whole frames.
export const SIDE_BODY = sprite(`
  ..........bbbbbbbbbb..........
  ........bbhhhhbbbbbbbb........
  ......bbhhhhhhhbbbbbbbbb......
  .....bbhhhhhbbbbbbbbbbbbb.....
  ....bbhhhbbbbbbbbbbbbbbbbb....
  ...bbhhbbbbbbbbbbbbbbbbbbbs...
  ..bbbhbbbbbbbbbbbbbbbbbbbbbs..
  ..bbhhbbbbbbbbbbbbbbbbbbbbbs..
  .bbbhbbbbbbbbbbbbbbbbbbbbbbbs.
  .bbbhbbbbbbbbbbbbbbbbbbbbbbbs.
  .bbbbbbbbbbbbbbbbbbbbbbbbbbbs.
  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbs
  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  bbbbbbbbbbbbbbbbbbbbwwwwwwbbbb
  bbbbbbbbbbbbbbbbbbwwwcccccwwbb
  bbbbbbbbbbbbbbbbbwwccccccccccw
  bbbbbbbbbbbbbbbbwccccccccccccc
  bbbbbbbbbbbbbbbwcccccccccccccc
  bbbbbbbbbbbbbbbwcccccccccccccc
  bbbbbbbbbbbbbbwccccccccccccccq
  sbbbbbbbbbbbbbwccccccccccccccq
  .sbbbbbbbbbbbbwcccccccccccccq.
  .ssbbbbbbbbbbbwccccccccccccqq.
  ..ssbbbbbbbbbbbwcccccccccqqq..
  ...sssbbbbbbbbbwccccccccqqq...
  ....sssssbbbbbbbwccccqqqqq....
  ......ssssssbbbbqqqqqqqq......
  .........sssssssqqqqq.........
`);

export const SIDE_BEAK = sprite(`
  yyy.
  aaar
  rr..
`);

export const SIDE_TAIL = sprite(`
  hbb....
  .sbbb..
  ..ssbbb
  ...ssss
`);

// Flap cycle: the raised wing clears the back so the flap reads in silhouette.
const SIDE_WING_DOWN = sprite(`
  ....hhbb.
  ..hhbbbbs
  .hbbbbsss
  .bbbssss.
  .bbsss...
  ..ss.....
`);
const SIDE_WING_MID = sprite(`
  .....hhhbbb.
  ..hhbbbbbbss
  hbbbbbbbsss.
  .ssss.......
`);
const SIDE_WING_UP = sprite(`
  h..h....
  hb.hb.h.
  hbbhbbhb
  .hbbbbbb
  ..bbbbbs
  ..bbbbss
  ...bbss.
  ....ss..
`);
export const SIDE_WING = {
  down: { s: SIDE_WING_DOWN, x: 4, y: 14 },
  mid: { s: SIDE_WING_MID, x: -3, y: 12 },
  up: { s: SIDE_WING_UP, x: 3, y: 0 },
};

export const SIDE_FEET = {
  planted: sprite(`
    .a...a..
    .a...a..
    .aaa.aaa
  `),
  tucked: sprite(`
    .a...a..
    aa..aa..
  `),
};
