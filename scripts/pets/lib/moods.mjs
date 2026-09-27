// The nine atlas rows for a front-facing creature, shared by every species
// except the bird (which keeps its own rows in species/bird/rows.mjs).
//
// A species passes:
//   front(o)  draws one front-facing frame. Options every species handles:
//               lift     move the whole pet up (jumping)
//               squash   rows removed (>0) or added (<0) below the face
//               lean     head shift in x
//               eyes     'open' | 'half' | 'blink' | 'happy' | 'up' | 'teary'
//               eyeDx, eyeDy  look direction
//               armL, armR    'rest' | 'flat' | 'diag' | 'up' | 'type' | 'type2' | 'droop' | 'hold'
//               droop    ears (or the species' equivalent) hang down (failed)
//               props(geo) => { back, face, mid, front } extra groups
//             `geo` gives the frame's layout: at least { top, fx, fy } (head top
//             and face origin) plus whatever the species' anchors need.
//   run()     one stride facing right, 4 frames; the row plays it twice and
//             running-left mirrors it.
//   anchors   where the mood props go:
//               ask: [x, y]         "?" bubble (waiting)
//               puff: [x, y]        thought puff (working)
//               cloud: [x, y], drops: [x, x]   storm cloud and its two rain streaks (failed)
//               laptop(geo) => [x, y]          laptop (working)
//               lens(geo) => [x, y]            magnifying glass (reviewing)
//             bigEye: the eye seen through the lens (defaults to the bird's)
//   touch     optional per-mood extra options, (i) => ({...}), for the
//             species' personality: { idle, waving, jumping, failed, waiting, working, review }
import { Layer } from './engine.mjs';
import * as P from './props.mjs';

export const part = (s, x, y) => (s ? { s, x, y } : null);
export const group = (...parts) => ({ parts });

export function mirrorLayer(layer) {
  const out = new Layer(layer.w, layer.h);
  for (let y = 0; y < layer.h; y++) {
    for (let x = 0; x < layer.w; x++) out.set(x, y, layer.get(layer.w - 1 - x, y));
  }
  return out;
}

const none = () => ({});

export function moodRows(sp) {
  const touch = { idle: none, waving: none, jumping: none, failed: none, waiting: none, working: none, review: none, ...sp.touch };
  const A = sp.anchors;
  const bigEye = sp.bigEye ?? P.BIG_EYE;
  const frames = (mood, list) => list.map((o, i) => sp.front({ ...o, ...touch[mood](i, o) }));

  const idle = () =>
    frames('idle', [{}, { eyes: 'half' }, { eyes: 'blink' }, { eyes: 'half' }, { squash: 1 }, { squash: 1 }]);

  const waving = () =>
    frames('waving', [
      { armL: 'flat' },
      { armL: 'up', eyes: 'happy', lean: -1 },
      { armL: 'diag', eyes: 'happy', lean: -1 },
      { armL: 'flat', eyes: 'happy' },
    ]);

  const jumping = () =>
    frames('jumping', [
      { squash: 2, eyes: 'blink' },
      { lift: 4, squash: -1, armL: 'diag', armR: 'diag' },
      { lift: 6, armL: 'flat', armR: 'flat', eyes: 'happy' },
      { lift: 3, armL: 'up', armR: 'up' },
      { squash: 1 },
    ]);

  const failed = () => {
    const lean = [0, -1, -1, 0, 0, 1, 1, 0];
    const cloudDy = [0, 0, 1, 1, 0, 0, 1, 1];
    // two rain streaks, out of phase; drops slip behind the head
    const dropA = [0, 2, 4, null, 0, 2, 4, null];
    const dropB = [4, null, 0, 2, 4, null, 0, 2];
    const [cx, cy] = A.cloud;
    const [xa, xb] = A.drops;
    return frames(
      'failed',
      lean.map((l, i) => ({
        squash: 2,
        droop: true,
        armL: 'droop',
        armR: 'droop',
        eyes: 'teary',
        lean: l,
        props: () => ({
          back: [
            group(part(P.CLOUD, cx, cy + cloudDy[i])),
            dropA[i] == null ? null : group(part(P.DROP, xa, cy + 9 + dropA[i] + cloudDy[i])),
            dropB[i] == null ? null : group(part(P.DROP, xb, cy + 9 + dropB[i] + cloudDy[i])),
          ],
        }),
      })),
    );
  };

  const waiting = () => {
    const [ax, ay] = A.ask;
    const ask = (dy) => () => ({ back: [group(part(P.ASK, ax, ay + dy))] });
    const base = { armL: 'up', eyes: 'up', eyeDy: -1 };
    return frames('waiting', [
      { ...base, props: ask(0) },
      { ...base, squash: 1, props: ask(0) },
      { ...base, squash: -1, props: ask(-1) },
      { ...base, props: ask(-1) },
      { ...base, eyes: 'blink', props: ask(0) },
      { ...base, props: ask(0) },
    ]);
  };

  const working = () => {
    const [px, py] = A.puff;
    const laptop = (dots) => (g) => {
      const [lx, ly] = A.laptop(g);
      return {
        // screen light spilling onto the belly just above the lid
        mid: [{ parts: [part(P.SCREEN_GLOW, lx + 1, ly - 1)], outline: false }],
        front: [group(part(P.LAPTOP, lx, ly))],
        back: [group(part(P.PUFFS[dots], px, py))],
      };
    };
    const f = (armL, armR, squash, dots) => ({ eyeDy: 1, armL, armR, squash, props: laptop(dots) });
    return frames('working', [
      f('type', 'type2', 0, 1),
      f('type2', 'type', 1, 2),
      f('type', 'type2', 0, 3),
      f('type2', 'type', 1, 3),
      f('type', 'type2', 0, 0),
      f('type2', 'type', 1, 1),
    ]);
  };

  const review = () => {
    const scan = [-1, -1, 0, 1, 1, 0];
    const nod = [0, 0, 0, 0, 1, 0];
    return frames(
      'review',
      scan.map((dx, i) => ({
        eyeDx: dx,
        squash: nod[i],
        armR: 'hold',
        props: (g) => {
          // the lens stays in the paw; the eye behind it scans with the face
          const [lx, ly] = A.lens(g);
          return {
            mid: [
              group(part(P.HANDLE, lx + 8, ly + 8)),
              group(part(P.LENS, lx, ly), part(bigEye, lx + 2 + dx, ly + 1)),
            ],
          };
        },
      })),
    );
  };

  const runRight = () => {
    const stride = sp.run();
    return [...stride, ...stride];
  };

  return [
    { name: 'idle', frames: idle },
    { name: 'running-right', frames: runRight },
    { name: 'running-left', frames: () => runRight().map(mirrorLayer) },
    { name: 'waving', frames: waving },
    { name: 'jumping', frames: jumping },
    { name: 'failed', frames: failed },
    { name: 'waiting', frames: waiting },
    { name: 'running', frames: working },
    { name: 'review', frames: review },
  ];
}
