// Colour roles used by the sprite maps, and per-pet palettes.
//
// Character roles (swapped per pet):
//   o outline        h body light   b body base    s body shadow   d body deep
//   w belly light    c belly base   q belly shadow
//   y beak light     a beak base    r beak shadow  O beak/feet outline
//   e eye            p blush
// Shared roles (same for every pet): see SHARED below.

const SHARED = {
  W: '#ffffff', // specular highlight, speech bubble
  // wood (branch)
  1: '#c89a66',
  2: '#9a6f45',
  3: '#6e4b2e',
  4: '#3d2817',
  // leaf
  g: '#9bd66a',
  G: '#5aa54a',
  n: '#1f4d2a',
  // metal (laptop, magnifier rim)
  L: '#e3e7ee',
  M: '#aab3c0',
  N: '#6c7686',
  9: '#262c3a',
  // screen
  K: '#1d2433',
  x: '#8ee6a0', // code green
  z: '#7cc4ff', // code blue / rain
  u: '#ffb35c', // code orange
  // glass
  l: '#cfeeff',
  // paper
  P: '#fffaf0',
  Z: '#e2d6bd',
  // storm cloud
  5: '#d5dbe3',
  6: '#a3adba',
  7: '#6e7888',
  8: '#3a4150',
  // alert red / amber (speech marks)
  R: '#ec4d4d',
  X: '#8f1f2a',
  A: '#f5b43c',
  V: '#8a5a12',
  // thought puff
  F: '#ffffff',
  f: '#c8ced8',
  // tear
  t: '#9fdcff',
};

export const PETS = {
  perch: {
    displayName: 'Perch',
    description: 'A round little teal songbird who keeps an eye on your Claude Code sessions.',
    colors: {
      o: '#15383c',
      h: '#72d8c6',
      b: '#3aa39a',
      s: '#28767a',
      d: '#1d5559',
      w: '#fffdf3',
      c: '#f5e5c3',
      q: '#d8bc8e',
      y: '#ffd27a',
      a: '#f39a36',
      r: '#c2612a',
      O: '#5c2a12',
      e: '#18222d',
      p: '#f4958a',
    },
  },
  ember: {
    displayName: 'Ember',
    description: 'A fiery little songbird with a warm glow and a short fuse for failing tests.',
    colors: {
      o: '#3e1512',
      h: '#ffa25e',
      b: '#e6552e',
      s: '#b13526',
      d: '#7e2320',
      w: '#fffbe3',
      c: '#ffe9a6',
      q: '#e9c26b',
      y: '#fff0a0',
      a: '#ffc93c',
      r: '#c98a1e',
      O: '#5a3408',
      e: '#26100f',
      p: '#ffb08a',
    },
  },
  plum: {
    displayName: 'Plum',
    description: 'A calm violet songbird who reads every diff twice before you do.',
    colors: {
      o: '#271541',
      h: '#c8a0f2',
      b: '#9362d1',
      s: '#6a42a3',
      d: '#4b2c7a',
      w: '#fdf9ff',
      c: '#eadcff',
      q: '#bfa6e3',
      y: '#ffd27a',
      a: '#f39a36',
      r: '#c2612a',
      O: '#5c2a12',
      e: '#1e1430',
      p: '#f59ab8',
    },
  },
};

export function paletteFor(id) {
  return { ...SHARED, ...PETS[id].colors };
}

// Which outline colour each fill role gets when auto-outlined.
export const OUTLINE_OF = {
  default: 'o',
  y: 'O',
  a: 'O',
  r: 'O',
  1: '4',
  2: '4',
  3: '4',
  g: 'n',
  G: 'n',
  L: '9',
  M: '9',
  N: '9',
  K: '9',
  x: '9',
  z: '9',
  u: '9',
  l: '9',
  P: '4',
  Z: '4',
  5: '8',
  6: '8',
  7: '8',
  R: 'X',
  A: 'V',
  F: '8',
  f: '8',
  W: '8',
  t: 'o',
};

// Roles that are already line pixels and never spawn more outline.
export const LINE_ROLES = new Set(['o', 'O', '4', 'n', '9', '8', 'X', 'V', 'e']);
