// Limbs shared by the pets that sit facing you (see sitter.mjs): a front
// leg drawn inside the torso at rest, the posed front legs (they double as
// arms), and a hind paw. The paw role ('k' in these maps) is swapped per
// species, so the fox gets dark socks and the cat pale mittens.
import { sprite, recolor } from './engine.mjs';

// A front leg at rest, drawn inside the torso's outline: a shaded edge
// against the haunch, and a dark sock for the paw.
const LEG_IN = sprite(`
  sbbb
  sbbb
  sbbb
  sbbb
  sbbb
  skkk
  kkkk
  kkkk
`);

const HIND_PAW = sprite(`
  .kkk
  kkkk
`);

// Front legs double as arms. x, y: offsets from the torso's top-left.
const LEG_FLAT = sprite(`
  .kk.....
  kkkbbbb.
  kkkbbbbs
  .kbbbbs.
`);
const LEG_DIAG = sprite(`
  .kk...
  kkkk..
  kkkbb.
  .bbbbs
  ..bbbs
  ...bbs
`);
const LEG_UP = sprite(`
  .kk.
  kkkk
  hbbs
  hbbs
  hbbs
  hbbs
  .bs.
`);
const LEG_TYPE = sprite(`
  .hbb
  .hbb
  hbbs
  hbbs
  hbbs
  bbss
`);
const LEG_TYPE2 = sprite(`
  .hbb
  hbbs
  hbbs
  bbss
`);
const LEG_HOLD = sprite(`
  kk....
  kkbbb.
  .bbbbs
  ..bbbs
`);
const LEG_POSES = {
  flat: { s: LEG_FLAT, x: -3, y: 2 },
  diag: { s: LEG_DIAG, x: -2, y: -4 },
  up: { s: LEG_UP, x: -5, y: -9 },
  type: { s: LEG_TYPE, x: 5, y: 1 },
  type2: { s: LEG_TYPE2, x: 5, y: 0 },
  hold: { s: LEG_HOLD, x: 0, y: -2 },
};

// Legs seen from the side (running), facing right.
const SIDE_LEGS = {
  down: sprite(`
    bbb
    bbb
    bbb
    kkk
    kkk
  `),
  fwd: sprite(`
    bb...
    bbb..
    .bbb.
    ..kkk
    ...kk
  `),
  back: sprite(`
    ...bb
    ..bbb
    .bbb.
    kkk..
    kk...
  `),
  tuck: sprite(`
    bbb
    bkk
    .kk
  `),
};

/** The limbs with the paws in the given colour role. x, y of each pose: offsets from the torso's top-left. */
export function legSet({ paw = 'k' } = {}) {
  const tint = (s) => (paw === 'k' ? s : recolor(s, { k: paw }));
  return {
    legIn: tint(LEG_IN),
    hindPaw: tint(HIND_PAW),
    legPoses: Object.fromEntries(Object.entries(LEG_POSES).map(([k, p]) => [k, { ...p, s: tint(p.s) }])),
    sideLegs: Object.fromEntries(Object.entries(SIDE_LEGS).map(([k, s]) => [k, tint(s)])),
  };
}
