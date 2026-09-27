// The cat (Miso): charcoal grey with a pale muzzle and mittens, amber eyes
// with slit pupils, whiskers, short ears and a thin tail with a hook at the tip.
// Roles: o outline, h/b/s/d charcoal light/base/shadow/deep, w/c/q pale grey
// light/base/shadow (muzzle, chest, paws), i amber iris, e pupils and mouth,
// k inner ear, p nose and blush.
import { sprite, sym, edgeShade, flipX, recolor, eyePair, shear } from '../../lib/engine.mjs';
import { legSet } from '../../lib/limbs.mjs';

const SHADE = { h: 'b', b: 's', w: 'c', c: 'q' };
const lit = (s) => edgeShade(s, SHADE);
/** A right-hand part from a left-hand one: mirrored, a touch darker. */
export const rightOf = (s) => lit(recolor(flipX(s), { h: 'b' }));

// ---------------------------------------------------------------- head
// Rounder and fuller in the cheeks than the fox, with a pale muzzle puff.
export const HEAD = lit(
  sym(
    `
    .....hhhhhhhhh
    ...hhhhhbbbbbb
    ..hhbbbbbbbbbb
    .hhbbbbbbbbbbb
    .hbbbbbbbbbbbb
    hhbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbww
    hbbbbbbbbbwwww
    .bbbbbbbbbwwww
    ..bbbbbbbbbwww
    ....bbbbbbbbbb
  `,
    { right: { h: 'b' } },
  ),
);
export const HEAD_W = HEAD.w; // 28

export const EAR = {
  up: sprite(`
    h.....
    hh....
    hkb...
    hkkb..
    hkkbb.
  `),
  // flattened out sideways (sulking)
  droop: sprite(`
    ...hb
    .hbkb
    hbkkb
    .bbbb
  `),
};

// Amber eyes with a slit pupil; closed eyes are drawn in pale grey so they
// show on the dark fur.
const EYE_SHAPES = {
  open: `
    .iii.
    iWeii
    iieii
    iieii
    .iii.`,
  // heavy-lidded: the smug look
  half: `
    .....
    eeeee
    iieii
    iieii
    .iii.`,
  blink: `
    .....
    .....
    .....
    c...c
    .ccc.`,
  happy: `
    .....
    .ccc.
    c...c
    .....
    .....`,
  up: `
    .iii.
    iWeii
    iieii
    iieWi
    .iii.`,
  teary: `
    .iii.
    iWeii
    iieii
    ittti
    .ttt.`,
};
export const EYES = Object.fromEntries(Object.entries(EYE_SHAPES).map(([n, s]) => [n, eyePair(s, { fill: 'i' })]));

export const BIG_EYE = sprite(`
  .iiii.
  iWWeii
  iWieii
  iiieii
  iiieii
  iiieii
  .iiii.
`);

export const NOSE = sprite(`
  .pp.
  e..e
`);
export const BLUSH = sprite(`
  pp
`);
export const WHISKERS = sprite(`
  ccc.
  ....
  cccc
`);

// ---------------------------------------------------------------- body
export const TORSO = lit(
  sym(
    `
    ....hbbbbcc
    ...hbbbbbcc
    ..hbbbbbbcc
    ..hbbbbbbcc
    .hbbbbbbbcc
    .hbbbbbbbbc
    .hbbbbbbbbc
    hbbbbbbbbbb
    hbbbbbbbbbb
    hbbbbbbbbbb
    hbbbbbbbbbb
    bbbbbbbbbbb
    .bbbbbbbbbb
    ..bbbbbbbbb
  `,
    { right: { h: 'b' } },
  ),
);
export const TORSO_W = TORSO.w; // 22
export const SQUASH_ROWS = [4, 7, 2];

const LIMBS = legSet({ paw: 'w' });
export const LEG_IN = LIMBS.legIn;
export const HIND_PAW = LIMBS.hindPaw;
export const SIDE_LEG = LIMBS.sideLegs;
// One long arm reaching up past the head, leaning out: the big stretch.
const LEG_STRETCH = shear(
  sprite(`
    .ww.
    wwww
    wwww
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    hbbs
    .bs.
  `),
  -0.17,
);
export const LEG_POSES = {
  ...LIMBS.legPoses,
  stretch: { s: LEG_STRETCH, x: -4 - LEG_STRETCH.ox, y: -14 },
};

// ---------------------------------------------------------------- tail
// Thin, rising from behind the right haunch and hooked at the tip.
export const TAIL = lit(
  sprite(`
    .hbbb...
    hbbbbb..
    bbs.bbs.
    bs...bs.
    ......bs
    ......bs
    ......bs
    .....hbs
    .....hbs
    .....hbs
    ....hbbs
    ...hbbs.
    ..hbbs..
    .hbbs...
    hbbs....
    bbs.....
  `),
);
// lying flat along the floor (sulking)
export const TAIL_DOWN = lit(
  sprite(`
    ........hbb
    hbbbbbbbbbs
    .sssssssss.
  `),
);

// ================================================================ SIDE VIEW
export const SIDE_HEAD = lit(
  sprite(`
    ..h.....h......
    ..hb...hb......
    ..hkb.hkbb.....
    .hbbbbbbbbb....
    hbbbbbbbbbbb...
    hbbbbbbbbbbbb..
    hbbbbbbbbbbbbb.
    hbbbbbbbbbbbwww
    bbbbbbbbbbwwwwp
    bbbbbbbbbwwwww.
    .bbbbbbbbwwww..
    ..bbbbbbbbb....
    ....bbbbbb.....
  `),
);
export const SIDE_EYE = sprite(`
  iW
  ie
  ii
`);
export const SIDE_BODY = lit(
  sprite(`
    ...hhhhhhhhhh....
    .hhbbbbbbbbbbbb..
    hbbbbbbbbbbbbbbc.
    hbbbbbbbbbbbbbccc
    bbbbbbbbbbbbbbccc
    bbbbbbbbbbbbbbbcc
    .bbbbbbbbbbbbbbb.
    ..bbbbbbbbbbbbb..
    ....bbbbbbbbb....
  `),
);
// held high while running, tip curling back
export const SIDE_TAIL = lit(
  sprite(`
    hb....
    bs....
    .bs...
    .hbs..
    ..bs..
    ..hbs.
    ...bbs
    ....bs
  `),
);
