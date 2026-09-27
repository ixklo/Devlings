// Generator tests: node --test (run from scripts/pets with `npm test`).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PET_IDS } from '../lib/contract.mjs';
import { PETS } from '../lib/pets.mjs';
import { buildAtlas, encodePng, manifestOf } from '../lib/atlas.mjs';
import { validatePet, bundledPetDirs, checkBundledSet } from '../validate.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const petDir = (id) => path.join(ROOT, 'src-tauri', 'pets', id);
const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');

// SHA-256 of the bird atlases as shipped in v1.0.0, before the generator
// learned about species. The birds must never change by accident.
const BIRD_V1 = {
  perch: 'c5f844a763cacf3be7b36784b8d3be1c8ab909e5ede4c94933cd627b191b27aa',
  ember: '714682333ca6d767901e8770e808595ae62485ac36e32b1c50b00b50d150ad39',
  plum: '7f46af5fcf560698112b723e25ba33e8c41a9377a4be24f4589931e71ae8d543',
};

test('the bird atlases are byte-identical to v1.0.0', () => {
  for (const [id, hash] of Object.entries(BIRD_V1)) {
    assert.equal(PETS[id].species, 'bird');
    assert.equal(sha256(encodePng(buildAtlas(id))), hash, `generated ${id}`);
    assert.equal(sha256(fs.readFileSync(path.join(petDir(id), 'spritesheet.png'))), hash, `committed ${id}`);
  }
});

test('nine bundled pets: the three birds and six new species', () => {
  assert.deepEqual(PET_IDS, ['perch', 'ember', 'plum', 'fox', 'cat', 'axolotl', 'capybara', 'robot', 'ghost']);
  assert.deepEqual(Object.keys(PETS).sort(), [...PET_IDS].sort());
  const species = new Set(PET_IDS.map((id) => PETS[id].species));
  assert.deepEqual([...species], ['bird', 'fox', 'cat', 'axolotl', 'capybara', 'robot', 'ghost']);
  const names = PET_IDS.map((id) => PETS[id].displayName);
  assert.deepEqual(names, ['Perch', 'Ember', 'Plum', 'Pip', 'Miso', 'Nori', 'Bean', 'Bolt', 'Wisp']);
});

test('the committed files are what the generator builds', () => {
  for (const id of PET_IDS) {
    const committed = fs.readFileSync(path.join(petDir(id), 'spritesheet.png'));
    assert.ok(committed.equals(encodePng(buildAtlas(id))), `${id}: spritesheet.png is stale (run build.mjs)`);
    const manifest = JSON.parse(fs.readFileSync(path.join(petDir(id), 'pet.json'), 'utf8'));
    assert.deepEqual(manifest, manifestOf(id));
  }
});

test('every bundled pet passes the validator', () => {
  const dirs = bundledPetDirs();
  assert.deepEqual(checkBundledSet(dirs), []);
  for (const dir of dirs) {
    const { errors } = validatePet(dir);
    assert.deepEqual(errors, [], path.basename(dir));
  }
});

test('descriptions say what the pet is, in one line', () => {
  for (const id of PET_IDS) {
    const { description } = PETS[id];
    assert.match(description, /^An? [^\n]+\.$/, id);
    assert.ok(description.length <= 100, `${id}: ${description.length} characters`);
  }
});
