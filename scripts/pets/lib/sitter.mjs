// Front view for a pet that sits facing you: a head over a torso, with front
// legs that rest inside the torso's outline and get their own outline once
// they move (waving, typing, holding the lens). The fox, cat, capybara and
// axolotl are all drawn this way; each brings its own parts (the rig).
//
// rig = {
//   floor        lowest fill row of the torso (outlines land one row lower)
//   head         head sprite; headX its left edge; overlap: head rows that hang over the torso
//   ears         { up, droop, ... }: { s, x, y } from the head's top-left (left ear; the right
//                mirrors). o.ears picks one by name; otherwise o.droop picks 'droop', else 'up'.
//   eyes         { open, half, ... } left/right pairs; eyeAt: { lx, rx, y } from the head's top-left
//   face         [{ s, x, y }] nose, mouth, blush... from the head's top-left (they follow the gaze)
//   torso        torso sprite; torsoX its left edge; squashRows rows to drop or double
//   legIn        { s, x }: a resting front leg inside the torso, x from the torso's left, or
//                null when the species draws 'rest' and 'droop' as posed legs instead
//   legPoses     { pose: { s, x, y, under? } } from the torso's top-left; `under` tucks the
//                leg behind the head (a snout that would otherwise be covered)
//   hindPaws     { s, x, dy } or null: left hind paw, x from the torso's left, dy up from the floor
//   tails        { name: sprite } and tailAt(torso geometry) => [x, y] for the tail's base
//   rightOf(s)   mirror a left part to the right side (with the species' shading)
//   extras(o, geo) optional: { back, head, front } extra groups/parts for personality touches
// }
import { compose, dropRows, dupRows } from './engine.mjs';
import { OUTLINE_OF, LINE_ROLES } from './palettes.mjs';
import { part, group } from './moods.mjs';

const AT_REST = new Set(['rest', 'droop']);

export function makeSitter(rig) {
  const outlineOf = { ...OUTLINE_OF, ...rig.outlineOf };
  const lineRoles = rig.lineRoles ? new Set([...LINE_ROLES, ...rig.lineRoles]) : LINE_ROLES;
  const squashed = (s, squash) => {
    if (squash > 0) return dropRows(s, rig.squashRows.slice(0, squash));
    if (squash < 0) return dupRows(s, rig.squashRows.slice(0, -squash));
    return s;
  };
  const tw = rig.torso.w;
  const hw = rig.head.w;

  function leg(pose, side, tTop, under) {
    const p = rig.legPoses[pose];
    if (!p || Boolean(p.under) !== under) return null;
    if (side === 'l') return group(part(p.s, rig.torsoX + p.x, tTop + p.y));
    return group(part(rig.rightOf(p.s), rig.torsoX + tw - p.x - p.s.w, tTop + p.y));
  }

  return function front(o = {}) {
    const lift = o.lift ?? 0;
    const torso = squashed(rig.torso, o.squash ?? 0);
    const tBottom = rig.floor - lift;
    const tTop = tBottom - torso.h + 1;
    const top = tTop + rig.overlap - (rig.head.h - 1);
    const hx = rig.headX + (o.lean ?? 0);
    const fx = hx + (o.eyeDx ?? 0);
    const eyeY = top + rig.eyeAt.y + (o.eyeDy ?? 0);
    const eyes = rig.eyes[o.eyes ?? 'open'];
    const ear = rig.ears[o.ears ?? (o.droop ? 'droop' : 'up')];
    const armL = o.armL ?? 'rest';
    const armR = o.armR ?? 'rest';
    const geo = { top, fx: hx, fy: top, hx, eyeY, tTop, tBottom, lift };
    const extra = o.props ? o.props(geo) : {};
    const touch = rig.extras ? rig.extras(o, geo) : {};
    const tail = rig.tails[o.tail ?? 'rest'];
    const [tailX, tailY] = rig.tailAt({ tTop, tBottom, tail });
    const legInY = rig.legIn ? tBottom - rig.legIn.s.h + 1 : 0;
    const restIn = (arm) => rig.legIn && AT_REST.has(arm);
    const pawY = tBottom - (rig.hindPaws?.dy ?? 0);

    const groups = [
      ...(extra.back ?? []),
      ...(touch.back ?? []),
      tail && group(part(tail, tailX - (tail.ox ?? 0), tailY)),
      group(
        part(torso, rig.torsoX, tTop),
        restIn(armL) ? part(rig.legIn.s, rig.torsoX + rig.legIn.x, legInY) : null,
        restIn(armR) ? part(rig.rightOf(rig.legIn.s), rig.torsoX + tw - rig.legIn.x - rig.legIn.s.w, legInY) : null,
      ),
      rig.hindPaws &&
        group(
          part(rig.hindPaws.s, rig.torsoX + rig.hindPaws.x, pawY),
          part(rig.rightOf(rig.hindPaws.s), rig.torsoX + tw - rig.hindPaws.x - rig.hindPaws.s.w, pawY),
        ),
      leg(armL, 'l', tTop, true),
      leg(armR, 'r', tTop, true),
      group(
        ear && part(ear.s, hx + ear.x, top + ear.y),
        ear && part(rig.rightOf(ear.s), hx + hw - ear.x - ear.s.w, top + ear.y),
        part(rig.head, hx, top),
        o.eyes === 'none' ? null : part(eyes.l, fx + rig.eyeAt.lx, eyeY),
        o.eyes === 'none' ? null : part(eyes.r, fx + rig.eyeAt.rx, eyeY),
        ...rig.face.map((f) => part(f.s, fx + f.x, top + f.y)),
        ...(touch.head ?? []),
        ...(extra.face ?? []),
      ),
      ...(extra.mid ?? []),
      leg(armL, 'l', tTop, false),
      leg(armR, 'r', tTop, false),
      ...(touch.front ?? []),
      ...(extra.front ?? []),
    ];
    return compose(groups, outlineOf, lineRoles);
  };
}
