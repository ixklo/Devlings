// Every species the generator can draw. A species supplies one frame builder
// per atlas row (`rows`, in atlas order, each `{ name, frames: () => Layer[] }`);
// pets pick a species and a palette in lib/pets.mjs.
import bird from './bird/index.mjs';
import fox from './fox/index.mjs';
import cat from './cat/index.mjs';
import axolotl from './axolotl/index.mjs';
import capybara from './capybara/index.mjs';
import robot from './robot/index.mjs';
import ghost from './ghost/index.mjs';

export const SPECIES = Object.fromEntries([bird, fox, cat, axolotl, capybara, robot, ghost].map((s) => [s.name, s]));
