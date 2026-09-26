// Codex pet atlas rows (openai/skills hatch-pet contract), in atlas order.
export const ATLAS_ROWS = [
  { name: 'idle', frames: 6, durations: [280, 110, 110, 140, 140, 320] },
  { name: 'running-right', frames: 8, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
  { name: 'running-left', frames: 8, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
  { name: 'waving', frames: 4, durations: [140, 140, 140, 280] },
  { name: 'jumping', frames: 5, durations: [140, 140, 140, 140, 280] },
  { name: 'failed', frames: 8, durations: [140, 140, 140, 140, 140, 140, 140, 240] },
  { name: 'waiting', frames: 6, durations: [150, 150, 150, 150, 150, 260] },
  { name: 'running', frames: 6, durations: [120, 120, 120, 120, 120, 220] },
  { name: 'review', frames: 6, durations: [150, 150, 150, 150, 150, 280] },
];

// Rows whose feet should rest on the same baseline (everything but
// locomotion and jumping).
export const BASELINE_ROWS = ['idle', 'waiting', 'running', 'review', 'failed'];

export const PET_IDS = ['perch', 'ember', 'plum'];
