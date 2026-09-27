// The robot (Bolt): light steel, a rounded head with a dark blue visor where
// its eyes glow, ear bolts, an antenna with a light, a chest panel with three
// status lights, tube arms and short legs.
// Roles: o outline, h/b/s/d steel light/base/shadow/deep, k/j visor and its
// glare, e glowing eyes, p blush, y lit antenna light (outlined with O),
// x/z/u chest lights on a K screen (shared screen colours).
import { sprite, sym, edgeShade, flipX, recolor, shear } from '../../lib/engine.mjs';
import { legSet } from '../../lib/limbs.mjs';

const SHADE = { h: 'b', b: 's' };
const lit = (s) => edgeShade(s, SHADE);
/** A right-hand part from a left-hand one: mirrored, a touch darker. */
export const rightOf = (s) => lit(recolor(flipX(s), { h: 'b', j: 'k' }));

// ---------------------------------------------------------------- head
export const HEAD = lit(
  sym(
    `
    ...hhhhhhhhhh
    .hhhhhbbbbbbb
    .hbbbbbbbbbbb
    hhbbbbbbbbbbb
    hbbkkkkkkkkkk
    hbkkjjkkkkkkk
    hbkjkkkkkkkkk
    hbkkkkkkkkkkk
    hbkkkkkkkkkkk
    hbkkkkkkkkkkk
    hbbkkkkkkkkkk
    hbbbbbbbbbbbb
    hbbbbbbbbbbbb
    .bbbbbbbbbbbb
    ..bbbbbbbbbbb
  `,
    { right: { h: 'b', j: 'k' } },
  ),
);
export const HEAD_W = HEAD.w; // 26

export const BOLT = sprite(`
  .hb
  hbs
  hds
  hbs
  .bs
`);

// Antenna: a stalk with a light on top; `on` picks the light's colour.
const antenna = (light) =>
  recolor(
    sprite(`
      .L.
      LLL
      .L.
      .d.
      .d.
    `),
    { L: light },
  );
export const ANTENNA = { on: antenna('y'), off: antenna('s') };
// bent over (failed)
export const ANTENNA_DROOP = sprite(`
  .s...
  sss..
  .sdd.
  ...d.
  ...d.
`);

// Eyes glow on the visor.
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
    ....`,
  blink: `
    ....
    ....
    eeee
    ....`,
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
    eeee
    .tt.`,
};
export const EYES = Object.fromEntries(
  Object.entries(EYE_SHAPES).map(([n, src]) => {
    const l = sprite(src);
    return [n, { l, r: l }];
  }),
);
export const BIG_EYE = sprite(`
  kkkkkk
  keeeek
  eWeeee
  eeeeee
  eeeeee
  keeeek
  kkkkkk
`);

export const MOUTH = sprite(`
  d....d
  .dddd.
`);
export const BLUSH = sprite(`
  pp
`);

// ---------------------------------------------------------------- body
export const TORSO = lit(
  sym(
    `
    ..hhhhhhhh
    .hbbbbbbbb
    hbbbbbbbbb
    hbbddddddd
    hbbdKKKKKK
    hbbdKKKKKK
    hbbbbbbbbb
    hbbbbbbbbb
    .bbbbbbbbb
    ..bbbbbbbb
    ....hbbs..
    ....hbbs..
  `,
    { right: { h: 'b' } },
  ),
);
export const TORSO_W = TORSO.w; // 20
export const SQUASH_ROWS = [7, 6, 2];
export const PANEL_LIGHTS = sprite(`
  x.z.u
`);

export const FOOT = sprite(`
  .ddd
  dddd
`);

const LIMBS = legSet({ paw: 'd' });
// Arms hang at its sides at rest; the other poses are the shared ones with steel hands.
const ARM = sprite(`
  hb.
  hbs
  hbs
  hbs
  hbs
  hbs
  ddd
  ddd
  .d.
`);
export const LEG_POSES = {
  ...LIMBS.legPoses,
  rest: { s: ARM, x: -3, y: 1 },
  droop: { s: ARM, x: -2, y: 2 },
};

// ================================================================ SIDE VIEW
export const SIDE_HEAD = lit(
  sprite(`
    ....hhhhhhhhhhhhhh..
    ..hhhhbbbbbbbbbbbbb.
    .hhbbbbbbbbbbbbbbbbb
    .hbbbbbbbbbbbbbbbbbb
    hhbbbbbbbbbbbkkkkkkk
    hbbbbbbbbbbbkkjjkkkk
    hbbbbbbbbbbbkjkkkkkk
    hbbbbbbbbbbbkkkkkkkk
    hbbbbbbbbbbbkkkkkkkk
    hbbbbbbbbbbbkkkkkkkk
    hbbbbbbbbbbbbkkkkkkk
    hbbbbbbbbbbbbbbbbbbb
    hbbbbbbbbbbbbbbbbbbb
    .bbbbbbbbbbbbbbbbbbb
    ..bbbbbbbbbbbbbbbbb.
  `),
);
export const SIDE_BOLT = sprite(`
  .hb.
  hdds
  hdds
  .bs.
`);
export const SIDE_EYE = sprite(`
  ee
  We
  ee
`);
export const SIDE_TORSO = lit(
  sprite(`
    .hhhhhhhhhhhh.
    hbbbbbbbbbbbbb
    hbbbbbbbbbdddd
    hbbbbbbbbbdKKK
    hbbbbbbbbbdKKK
    hbbbbbbbbbbbbb
    hbbbbbbbbbbbbb
    .bbbbbbbbbbbbb
    ..bbbbbbbbbbb.
  `),
);
const SIDE_LEG = sprite(`
  hbs.
  hbs.
  hbs.
  hbs.
  dddd
`);
export const SIDE_LEGS = {
  down: SIDE_LEG,
  fwd: shear(SIDE_LEG, -0.5),
  back: shear(SIDE_LEG, 0.5),
};
export const SIDE_ARM = {
  down: ARM,
  fwd: shear(ARM, -0.4),
  back: shear(ARM, 0.4),
};
