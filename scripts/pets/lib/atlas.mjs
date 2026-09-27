// Builds a pet's atlas in memory: its species draws the frames, its palette
// colours them. No files are written here (see build.mjs).
import { ATLAS_W, ATLAS_H, CELL_W, CELL_H, newImage, paintLayer } from './engine.mjs';
import { ATLAS_ROWS } from './contract.mjs';
import { PETS } from './pets.mjs';
import { paletteFor } from './palettes.mjs';
import { SPECIES } from '../species/index.mjs';

const framesBySpecies = new Map();

/** Every frame of a species, row by row, checked against the atlas contract. Built once per species. */
export function speciesFrames(name) {
  if (framesBySpecies.has(name)) return framesBySpecies.get(name);
  const species = SPECIES[name];
  if (!species) throw new Error(`Unknown species '${name}'`);
  const frames = ATLAS_ROWS.map((row, r) => {
    const spec = species.rows[r];
    if (spec?.name !== row.name) throw new Error(`${name} row ${r}: expected ${row.name}, got ${spec?.name}`);
    const layers = spec.frames();
    if (layers.length !== row.frames) {
      throw new Error(`${name} row ${row.name}: expected ${row.frames} frames, got ${layers.length}`);
    }
    return layers;
  });
  framesBySpecies.set(name, frames);
  return frames;
}

export function petOf(id) {
  const pet = PETS[id];
  if (!pet) throw new Error(`Unknown pet '${id}'`);
  return pet;
}

/** The 1536x1872 atlas of one pet as a pngjs image. */
export function buildAtlas(id) {
  const pet = petOf(id);
  const palette = paletteFor(pet);
  const img = newImage(ATLAS_W, ATLAS_H);
  speciesFrames(pet.species).forEach((row, r) => {
    row.forEach((layer, c) => paintLayer(img, layer, palette, c * CELL_W, r * CELL_H));
  });
  return img;
}


/** The pet.json manifest of one pet. */
export function manifestOf(id) {
  const pet = petOf(id);
  return { id, displayName: pet.displayName, description: pet.description, spritesheetPath: 'spritesheet.png' };
}
