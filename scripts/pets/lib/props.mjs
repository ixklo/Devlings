// Mood props shared by every species: the laptop (working), the thought puff,
// the "?" bubble (waiting), the storm cloud (failed) and the magnifying glass
// (reviewing). They use the shared colour roles in palettes.mjs, so they look
// the same whichever pet holds them.
import { sprite } from './engine.mjs';

// ================================================================ PROPS
// Laptop seen from behind (its screen faces the bird). The keyboard is
// hidden behind the lid; wing tips reach over the top edge to type.
export const LAPTOP = sprite(`
  .LLLLLLLLLLLLLLL.
  LLMMMMMMMMMMMMMMN
  LMMMMMMMMMMMMMMMN
  LMMMMMMMMMMMMMMMN
  LMMMMMMMggMMMMMMN
  LMMMMMMgGGgMMMMMN
  LMMMMMMMGgMMMMMMN
  LMMMMMMMMMMMMMMMN
  LMMMMMMMMMMMMMMMN
  NNNNNNNNNNNNNNNNN
`);
// light spilling over the top edge of the lid from the screen
export const SCREEN_GLOW = sprite(`
  .lllllllllllllll.
`);

// Thought puff with 0..3 dots.
const PUFF = `
  ...FFFF..FFF...
  .FFFFFFFFFFFFF.
  FFFFFFFFFFFFFFF
  FFFFFFFFFFFFFFF
  FFFFFFFFFFFFFFF
  .FFFFFFFFFFFFF.
  ..fFFFFfFFFFf..
  ...............
  F..............
`;
export const PUFFS = [0, 1, 2, 3].map((n) => {
  const s = sprite(PUFF);
  const rows = s.rows.map((r) => [...r]);
  for (let i = 0; i < n; i++) {
    for (const [dx, dy] of [[0, 0], [1, 0], [0, 1], [1, 1]]) rows[3 + dy][3 + i * 4 + dx] = '8';
  }
  return { ...s, rows: rows.map((r) => r.join('')) };
});

// Speech bubble with a question mark (waiting on the user).
export const ASK = sprite(`
  ..WWWWWWWW..
  .WWW8888WWW.
  WWW88WW88WWW
  WWWWWWW88WWW
  WWWWWW88WWWW
  WWWWW88WWWWW
  WWWWW88WWWWW
  WWWWWWWWWWWW
  .WWWW88WWWW.
  ..WWW88WWW..
  ...W........
  ..W.........
`);

// Small storm cloud (failed).
export const CLOUD = sprite(`
  ......555.........
  ....5556665..555..
  ...566666665566665
  ..5666666666666666
  .56666666666666667
  566666666666666677
  .6777766667776777.
  ..77777777777777..
`);
export const DROP = sprite(`
  z
  z
`);

// Magnifying glass: metal rim, glass with a glint, wooden handle.
export const LENS = sprite(`
  ...MMMM...
  ..MllllM..
  .MlWllllN.
  MlWlllllln
  Mllllllll9
  Mllllllll9
  MlllllllN9
  .NllllllN.
  ..NNlNNN..
  ...9999...
`);
export const HANDLE = sprite(`
  23..
  223.
  .223
  ..22
`);
export const BIG_EYE = sprite(`
  .eeee.
  eWWeee
  eWWeee
  eeeeee
  eeeeee
  eeeeee
  .eeee.
`);
