// Every species the generator can draw. A species supplies one frame builder
// per atlas row (`rows`, in atlas order, each `{ name, frames: () => Layer[] }`);
// pets pick a species and a palette in lib/pets.mjs.
import bird from './bird/index.mjs';

export const SPECIES = Object.fromEntries([bird].map((s) => [s.name, s]));
