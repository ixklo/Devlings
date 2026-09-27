//! The pet overlay window: click-through hit testing and on-screen placement.
//!
//! Design D9: the **sprite**, not the window, is kept inside the work area, so the pet can sit
//! flush against every edge and corner; the window is free to extend off-screen around it. Cards
//! (`.stage`) flip below the sprite when there isn't room above, and shift sideways near the left
//! or right edge. See `docs/specs/2026-09-26-perch-v1.0-design.md` D9.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    thread::Thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, WebviewWindow, WindowEvent};

use crate::{locks::lock, state::AppState};

pub const PET: &str = "pet";
pub const HIT_PADDING: f64 = 6.0;

// ---- Cursor poll rate (design D17) ----

/// Cursor poll interval while the cursor is on the pet window or near it.
pub const POLL_NEAR_MS: u64 = 33;
/// The longest interval, with the cursor far from the pet window.
pub const POLL_FAR_MS: u64 = 250;
/// In between, the interval grows with the distance at this rate (logical px per ms): the cursor
/// would have to move faster than about 2000 px/s to reach the window between two polls. Within
/// about 64 px the fastest rate applies.
const APPROACH_PX_PER_MS: f64 = 2.0;
/// How long the poll sleeps while the pet window is hidden or minimized, unless woken sooner.
const PAUSED_MS: u64 = 1000;
/// The cached window geometry is re-read after any move, resize or scale change; this is only a
/// backstop in case such an event is ever missed.
const GEOMETRY_MAX_AGE: Duration = Duration::from_secs(5);
/// Gap between the sprite and the work-area edges at the default position, in logical px.
pub const EDGE_MARGIN: f64 = 16.0;

// ---- Sprite/stage layout (logical CSS px; must stay in sync with pet.css and sprite/atlas.ts) ----

/// The sprite's cell size at scale 1 (`sprite/atlas.ts`'s `CELL_W`/`CELL_H`).
const CELL_W: f64 = 192.0;
const CELL_H: f64 = 208.0;
/// `.overlay`'s padding (pet.css).
const OVERLAY_PAD_TOP: f64 = 16.0;
const OVERLAY_PAD_BOTTOM: f64 = 14.0;
/// `.dock`'s gap between the sprite and the control bar (pet.css).
const DOCK_GAP: f64 = 2.0;
/// `.control-bar`'s outer height: 28px icon buttons + 3px padding top/bottom + 1px border top/bottom.
const BAR_HEIGHT: f64 = 36.0;
/// `.overlay`'s own gap between `.stage` and `.dock` (pet.css) — distinct from `.dock`'s own
/// `DOCK_GAP` between the sprite and the control bar.
const OVERLAY_GAP: f64 = 6.0;
/// Free space above the sprite needed to open cards upward; below this they open below it instead.
/// Two thresholds (enter lower than exit) give the flip hysteresis, so a sprite parked right at the
/// boundary doesn't flicker between layouts.
const FLIP_ENTER: f64 = 320.0;
const FLIP_EXIT: f64 = 380.0;

/// How `.stage` should lay out for the sprite's current on-screen spot: sent to the frontend as the
/// `pet-placement` event.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// Cards open below the sprite (flipped) instead of above it.
    pub cards_below: bool,
    /// Logical px to shift `.stage` sideways (a CSS transform), so cards stay on screen near a
    /// left/right edge even though the window itself may extend past it.
    pub shift_x: f64,
    /// Logical px `.stage` actually has on screen in this layout: at most its natural height, and
    /// at least 0. The window's top or bottom edge can be off-screen even when the sprite itself
    /// isn't (the flip only triggers once room drops below ~320-380px), so this is applied as
    /// `.stage`'s `max-height`, letting content shrink or scroll instead of being cut off.
    pub stage_room: f64,
}

/// The sprite's size in logical px at a given pet scale.
fn sprite_size(pet_scale: f64) -> (f64, f64) {
    (CELL_W * pet_scale, CELL_H * pet_scale)
}

/// The sprite's offset from the window's top-left, in logical px, for a given layout direction.
/// The sprite is always horizontally centered; vertically it sits at the bottom (dock last) in the
/// normal layout, or at the top (dock first, `order: -1` in pet.css) when cards are flipped below.
fn sprite_offset(cards_below: bool, pet_scale: f64, window: (f64, f64)) -> (f64, f64) {
    let (sw, sh) = sprite_size(pet_scale);
    let x = (window.0 - sw) / 2.0;
    let y = if cards_below { OVERLAY_PAD_TOP } else { window.1 - OVERLAY_PAD_BOTTOM - BAR_HEIGHT - DOCK_GAP - sh };
    (x, y)
}

/// Whether cards should open below the sprite, given how much room (logical px) is free above it
/// and the previous state (for hysteresis: entering "below" needs less room than leaving it).
fn next_cards_below(previously_below: bool, room_above: f64) -> bool {
    if previously_below { room_above <= FLIP_EXIT } else { room_above < FLIP_ENTER }
}

/// The stage's height when nothing constrains it: the window's height minus its own padding, the
/// gap to the dock, and the dock itself. Layout-independent — the flip only moves which end of the
/// window the dock sits at, not the window's total height or the dock's own height.
fn stage_natural_height(pet_scale: f64, window_h: f64) -> f64 {
    let (_, sh) = sprite_size(pet_scale);
    let dock_h = sh + DOCK_GAP + BAR_HEIGHT;
    (window_h - OVERLAY_PAD_TOP - OVERLAY_PAD_BOTTOM - OVERLAY_GAP - dock_h).max(0.0)
}

/// An interactive rectangle reported by the frontend, in window-relative CSS px.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HitRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Names elements whose hover the pet reacts to (the value of their `data-hit`).
    #[serde(default)]
    pub id: Option<String>,
}

/// A screen rectangle in physical px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The first padded rect under a physical cursor position, for a window at `origin` with `scale`.
fn hit_at(cursor: (f64, f64), origin: (i32, i32), scale: f64, rects: &[HitRect]) -> Option<&HitRect> {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let x = (cursor.0 - f64::from(origin.0)) / scale;
    let y = (cursor.1 - f64::from(origin.1)) / scale;
    rects.iter().find(|r| {
        x >= r.x - HIT_PADDING && x <= r.x + r.w + HIT_PADDING && y >= r.y - HIT_PADDING && y <= r.y + r.h + HIT_PADDING
    })
}

/// Whether a physical cursor position falls inside any padded rect.
pub fn cursor_hits(cursor: (f64, f64), origin: (i32, i32), scale: f64, rects: &[HitRect]) -> bool {
    hit_at(cursor, origin, scale, rects).is_some()
}

/// The id of the rect under the cursor, if that rect has one.
pub fn hovered_id(cursor: (f64, f64), origin: (i32, i32), scale: f64, rects: &[HitRect]) -> Option<String> {
    hit_at(cursor, origin, scale, rects).and_then(|r| r.id.clone())
}

/// Logical px from a physical cursor position to the nearest point of a physical window rect;
/// 0 inside it.
fn distance_to_window(cursor: (f64, f64), window: Area, scale: f64) -> f64 {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let (left, top) = (f64::from(window.x), f64::from(window.y));
    let (right, bottom) = (left + f64::from(window.w), top + f64::from(window.h));
    let dx = (left - cursor.0).max(cursor.0 - right).max(0.0);
    let dy = (top - cursor.1).max(cursor.1 - bottom).max(0.0);
    dx.hypot(dy) / scale
}

/// How long the cursor poll waits before its next look (design D17): 33 ms with the cursor inside
/// the pet window or within about 64 logical px of it, so hover and clicks stay instant. Farther
/// out the wait grows with the distance, up to 250 ms, so a quick approach is still caught before
/// the cursor gets there. An unknown cursor position keeps the fast rate.
pub fn poll_delay(cursor: (f64, f64), window: Area, scale: f64) -> Duration {
    let distance = distance_to_window(cursor, window, scale);
    if !distance.is_finite() {
        return Duration::from_millis(POLL_NEAR_MS);
    }
    let ms = (distance / APPROACH_PX_PER_MS).clamp(POLL_NEAR_MS as f64, POLL_FAR_MS as f64);
    Duration::from_millis(ms.round() as u64)
}

/// Whether a window counts as on screen: not hidden by Perch and not minimized. The cursor poll
/// runs, and a page renders, only while its window is on screen.
pub fn on_screen(hidden: bool, minimized: bool) -> bool {
    !hidden && !minimized
}

/// Windows reports a minimized window as resized to 0x0.
pub fn minimized_size(width: u32, height: u32) -> bool {
    width == 0 || height == 0
}

/// Clamps a horizontal span of `w` at `x` fully inside `[area_x, area_x + area_w]`: centered if
/// it's wider than the area, otherwise kept fully inside.
fn clamp_axis_x(area_x: i32, area_w: i32, x: i32, w: i32) -> i32 {
    if w > area_w { area_x + (area_w - w) / 2 } else { x.clamp(area_x, area_x + area_w - w) }
}

/// Moves a window of `size` fully inside `area`. A window taller than the area keeps its bottom edge
/// (where the pet sits) on screen; one wider than the area is centered.
pub fn clamp_into(area: Area, pos: (i32, i32), size: (i32, i32)) -> (i32, i32) {
    let x = clamp_axis_x(area.x, area.w, pos.0, size.0);
    let y = if size.1 > area.h {
        area.y + area.h - size.1
    } else {
        pos.1.clamp(area.y, area.y + area.h - size.1)
    };
    (x, y)
}

/// Bottom-right of the work area, inset by `margin`.
pub fn default_position(work: Area, size: (i32, i32), margin: i32) -> (i32, i32) {
    let pos = (work.x + work.w - size.0 - margin, work.y + work.h - size.1 - margin);
    clamp_into(work, pos, size)
}

fn overlap(a: Area, pos: (i32, i32), size: (i32, i32)) -> i64 {
    let w = (a.x + a.w).min(pos.0 + size.0) - a.x.max(pos.0);
    let h = (a.y + a.h).min(pos.1 + size.1) - a.y.max(pos.1);
    if w > 0 && h > 0 { i64::from(w) * i64::from(h) } else { 0 }
}

/// The work area a window of `size` at `pos` overlaps most. None when it overlaps none at all.
fn best_area(pos: (i32, i32), size: (i32, i32), areas: &[Area]) -> Option<Area> {
    let (best, amount) = areas.iter().map(|a| (*a, overlap(*a, pos, size))).max_by_key(|(_, o)| *o)?;
    (amount > 0).then_some(best)
}

/// Clamps a window into the work area it overlaps most. None when it is on no work area at all.
pub fn clamp_position(pos: (i32, i32), size: (i32, i32), areas: &[Area]) -> Option<(i32, i32)> {
    best_area(pos, size, areas).map(|a| clamp_into(a, pos, size))
}

/// Resolves the sprite's anchor to actually use, and whether that meant falling back to
/// `default` (a saved position whose monitor is gone, or no saved position at all) rather than
/// keeping the saved one, clamped into whichever work area it still overlaps (gate G3.5: recovery
/// from an off-screen saved position). Kept separate from `apply_sprite_position` so a caller like
/// `drag_pet_by` can still do nothing when a drag itself goes off every monitor, instead of
/// snapping to the default mid-drag.
fn resolve_sprite_anchor(saved: Option<(i32, i32)>, sprite_size: (i32, i32), areas: &[Area], default: (i32, i32)) -> ((i32, i32), bool) {
    match saved.and_then(|p| clamp_position(p, sprite_size, areas)) {
        Some(p) => (p, false),
        None => (default, true),
    }
}

/// Where to put the window (physical top-left) so the sprite sits at `sprite_anchor` (physical,
/// already clamped into `area`), and how `.stage` should lay out. `scale` converts pet.css's
/// logical layout constants to this monitor's physical px. `window_size` is the window's own
/// (fixed) physical outer size. `previously_below` carries the flip's hysteresis across calls.
fn place_sprite(
    sprite_anchor: (i32, i32),
    area: Area,
    scale: f64,
    window_size: (i32, i32),
    pet_scale: f64,
    previously_below: bool,
) -> ((i32, i32), Placement) {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let window_logical = (f64::from(window_size.0) / scale, f64::from(window_size.1) / scale);
    let room_above_logical = f64::from(sprite_anchor.1 - area.y) / scale;
    let cards_below = next_cards_below(previously_below, room_above_logical);
    let (ox, oy) = sprite_offset(cards_below, pet_scale, window_logical);
    let window_pos = (sprite_anchor.0 - (ox * scale).round() as i32, sprite_anchor.1 - (oy * scale).round() as i32);
    let shift_x_physical = clamp_axis_x(area.x, area.w, window_pos.0, window_size.0) - window_pos.0;
    let shift_x = f64::from(shift_x_physical) / scale;

    // How much of the stage's natural height is actually on screen: unflipped, from the work
    // area's top edge down to the stage's bottom (just above the dock); flipped, from the stage's
    // top (just below the dock) down to the work area's bottom edge.
    let (_, sh) = sprite_size(pet_scale);
    let natural = stage_natural_height(pet_scale, window_logical.1);
    let room_physical = if cards_below {
        let stage_top = f64::from(sprite_anchor.1) + (sh + DOCK_GAP + BAR_HEIGHT + OVERLAY_GAP) * scale;
        f64::from(area.y + area.h) - stage_top
    } else {
        let stage_bottom = f64::from(sprite_anchor.1) - OVERLAY_GAP * scale;
        stage_bottom - f64::from(area.y)
    };
    let stage_room = (room_physical / scale).clamp(0.0, natural);

    (window_pos, Placement { cards_below, shift_x, stage_room })
}

/// Converts a v0.2-era saved position (the *window's* top-left) to what a saved position means from
/// v1.0 on (the *sprite's* top-left), so an old saved spot still lands at the same visual spot (gate
/// G2.2). v0.2 never flipped the layout, so the migration always uses the un-flipped offset.
fn migrate_saved_position(window_pos: (i32, i32), scale: f64, pet_scale: f64, window_size: (i32, i32)) -> (i32, i32) {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let window_logical = (f64::from(window_size.0) / scale, f64::from(window_size.1) / scale);
    let (ox, oy) = sprite_offset(false, pet_scale, window_logical);
    (window_pos.0 + (ox * scale).round() as i32, window_pos.1 + (oy * scale).round() as i32)
}

fn work_area(m: &Monitor) -> Area {
    let r = m.work_area();
    Area { x: r.position.x, y: r.position.y, w: r.size.width as i32, h: r.size.height as i32 }
}

pub fn pet_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(PET)
}

fn outer_size(pet: &WebviewWindow) -> (i32, i32) {
    pet.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or((380, 600))
}

fn work_areas(pet: &WebviewWindow) -> Vec<Area> {
    pet.available_monitors().unwrap_or_default().iter().map(work_area).collect()
}

fn primary_or_first_monitor(pet: &WebviewWindow) -> Option<Monitor> {
    pet.primary_monitor().ok().flatten().or_else(|| pet.available_monitors().ok().and_then(|m| m.into_iter().next()))
}

/// The sprite's default anchor: the bottom-right of the primary work area, inset by `EDGE_MARGIN`.
fn default_sprite_anchor(pet: &WebviewWindow, pet_scale: f64) -> Option<(i32, i32)> {
    let monitor = primary_or_first_monitor(pet)?;
    let scale = monitor.scale_factor();
    let margin = (EDGE_MARGIN * scale).round() as i32;
    let size = sprite_size_physical(pet_scale, scale);
    Some(default_position(work_area(&monitor), size, margin))
}

/// The sprite's size in physical px, from its logical size and the monitor's scale factor.
fn sprite_size_physical(pet_scale: f64, scale: f64) -> (i32, i32) {
    let (w, h) = sprite_size(pet_scale);
    ((w * scale).round() as i32, (h * scale).round() as i32)
}

fn set_pos(pet: &WebviewWindow, pos: (i32, i32)) {
    let _ = pet.set_position(PhysicalPosition::new(pos.0, pos.1));
}

/// Saves the sprite's anchor (or clears it, to fall back to the default next time). A `Some` value
/// is always in the current (sprite-anchor) format, so it never needs migrating again.
fn remember(app: &AppHandle, pos: Option<(i32, i32)>) {
    let s = app.state::<AppState>();
    let mut c = lock(&s.config);
    c.pet_position = pos;
    if pos.is_some() {
        c.pet_position_migrated = true;
    }
    drop(c);
    s.save_config();
}

/// Clamps the sprite's anchor into the work area it mostly overlaps, moves the window so the
/// sprite lands there, remembers the flip state, and tells the frontend the new `.stage` layout.
/// Returns the sprite's clamped anchor (physical), for callers that persist it.
fn apply_sprite_position(app: &AppHandle, pet: &WebviewWindow, target: (i32, i32)) -> Option<(i32, i32)> {
    let s = app.state::<AppState>();
    let pet_scale = lock(&s.config).pet_scale;
    let scale = pet.scale_factor().ok()?;
    let areas = work_areas(pet);
    let sprite_size = sprite_size_physical(pet_scale, scale);
    let clamped = clamp_position(target, sprite_size, &areas).unwrap_or(target);
    let area = best_area(clamped, sprite_size, &areas)?;
    let previously_below = s.cards_below.load(Ordering::SeqCst);
    let (window_pos, placement) = place_sprite(clamped, area, scale, outer_size(pet), pet_scale, previously_below);
    set_pos(pet, window_pos);
    s.cards_below.store(placement.cards_below, Ordering::SeqCst);
    *lock(&s.placement) = Some(placement);
    let _ = app.emit_to(PET, "pet-placement", &placement);
    Some(clamped)
}

/// Startup placement: the saved sprite anchor clamped into a visible work area, else the default.
/// A v0.2 saved position (the window's top-left) is migrated to the sprite's top-left once, and
/// only then written back, so it isn't migrated again next launch.
pub fn place_pet(app: &AppHandle) {
    let Some(pet) = pet_window(app) else { return };
    let s = app.state::<AppState>();
    let (saved, migrated, pet_scale) = {
        let c = lock(&s.config);
        (c.pet_position, c.pet_position_migrated, c.pet_scale)
    };
    let mut just_migrated = false;
    let saved = match (saved, migrated) {
        (Some(p), false) => match pet.scale_factor() {
            Ok(scale) => {
                just_migrated = true;
                Some(migrate_saved_position(p, scale, pet_scale, outer_size(&pet)))
            }
            Err(_) => Some(p),
        },
        (Some(p), true) => Some(p),
        (None, _) => None,
    };
    let Some(default) = default_sprite_anchor(&pet, pet_scale) else { return };
    let Ok(scale) = pet.scale_factor() else { return };
    let sprite_size = sprite_size_physical(pet_scale, scale);
    let areas = work_areas(&pet);
    let (target, fell_back) = resolve_sprite_anchor(saved, sprite_size, &areas, default);
    if let Some(anchor) = apply_sprite_position(app, &pet, target) {
        // A dead saved spot (its monitor is gone) is worth re-saving so the next launch doesn't
        // retry it, same as a freshly-migrated value. Nothing saved at all is left alone, as
        // before D9, so an untouched install keeps recomputing the default if the screen changes.
        if just_migrated || (fell_back && saved.is_some()) {
            remember(app, Some(anchor));
        }
    }
}

/// Esc: sends the pet home (the default position), forgetting any saved spot.
pub fn reset_pet_position(app: &AppHandle) {
    let Some(pet) = pet_window(app) else { return };
    let pet_scale = lock(&app.state::<AppState>().config).pet_scale;
    if let Some(anchor) = default_sprite_anchor(&pet, pet_scale) {
        apply_sprite_position(app, &pet, anchor);
    }
    remember(app, None);
}

/// Moves the sprite by logical px, keeping it inside a work area, and flips/shifts `.stage` as
/// needed. Returns the sprite's new anchor (physical). Used on every pointer move while dragging,
/// so it doesn't save; the frontend saves when the move settles.
pub fn drag_pet_by(app: &AppHandle, dx: f64, dy: f64) -> Option<(i32, i32)> {
    let pet = pet_window(app)?;
    let s = app.state::<AppState>();
    let pet_scale = lock(&s.config).pet_scale;
    let cards_below = s.cards_below.load(Ordering::SeqCst);
    let (Ok(win_pos), Ok(scale)) = (pet.outer_position(), pet.scale_factor()) else { return None };
    let win_size = outer_size(&pet);
    let win_logical = (f64::from(win_size.0) / scale, f64::from(win_size.1) / scale);
    let (ox, oy) = sprite_offset(cards_below, pet_scale, win_logical);
    let current = (win_pos.x + (ox * scale).round() as i32, win_pos.y + (oy * scale).round() as i32);
    let target = (current.0 + (dx * scale).round() as i32, current.1 + (dy * scale).round() as i32);
    apply_sprite_position(app, &pet, target)
}

/// Nudges the pet by logical px (arrow keys) and saves the new spot.
pub fn move_pet_by(app: &AppHandle, dx: f64, dy: f64) {
    let Some(pos) = drag_pet_by(app, dx, dy) else { return };
    remember(app, Some(pos));
}

/// The pet window's geometry as the cursor poll last read it. Each read is a round trip to the main
/// thread, so it's re-read only when `AppState::pet_geometry_gen` says the window moved, resized or
/// changed scale (or, as a backstop, after `GEOMETRY_MAX_AGE`).
struct Geometry {
    generation: u64,
    read_at: Instant,
    origin: (i32, i32),
    size: (i32, i32),
    scale: f64,
}

fn read_geometry(pet: &WebviewWindow, generation: u64) -> Option<Geometry> {
    let origin = pet.inner_position().or_else(|_| pet.outer_position()).ok()?;
    let scale = pet.scale_factor().ok()?;
    Some(Geometry { generation, read_at: Instant::now(), origin: (origin.x, origin.y), size: outer_size(pet), scale })
}

static POLLER: OnceLock<Thread> = OnceLock::new();
/// Set while the poll is waiting longer than its fastest interval (the cursor is far away).
static POLL_IS_SLOW: AtomicBool = AtomicBool::new(false);

/// Wakes the cursor poll at once (the pet was just shown or restored), instead of at its next
/// paused check, so the first click lands.
pub fn wake_click_through() {
    if let Some(poller) = POLLER.get() {
        poller.unpark();
    }
}

/// Tells the cursor poll to re-read the pet window's geometry after a move, resize or scale change.
/// If the poll is in a slow wait, it's woken too: the window may have just jumped under the cursor
/// (Esc sends the pet home). Called on the main thread for every pet window event, so it only
/// touches atomics; during a drag the cursor is on the pet, so the poll is already fast.
pub fn on_pet_window_event(app: &AppHandle, event: &WindowEvent) {
    if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
        app.state::<AppState>().pet_geometry_gen.fetch_add(1, Ordering::SeqCst);
        if POLL_IS_SLOW.load(Ordering::SeqCst) {
            wake_click_through();
        }
    }
}

/// Polls the cursor and lets clicks fall through the pet window except over the reported rects.
///
/// Also emits `pet-pointer` with the hovered rect's id whenever it changes. The webview can't
/// track hover itself: once the window turns click-through it never sees the pointer leave.
///
/// Design D17: the rate adapts to how close the cursor is (`poll_delay`), the poll pauses while
/// the pet window is hidden or minimized, and the window's geometry is cached between moves, so
/// a steady poll costs one main-thread round trip (the cursor position) instead of three.
pub fn start_click_through(app: AppHandle) {
    let poller = std::thread::spawn(move || {
        let mut ignoring: Option<bool> = None;
        let mut hovered: Option<String> = None;
        let mut geometry: Option<Geometry> = None;
        let mut delay = Duration::from_millis(POLL_NEAR_MS);
        loop {
            // Parking, not sleeping, so `wake_click_through` can cut a wait short.
            std::thread::park_timeout(delay);
            POLL_IS_SLOW.store(false, Ordering::SeqCst);
            delay = Duration::from_millis(POLL_NEAR_MS);
            let s = app.state::<AppState>();
            if !on_screen(s.pet_hidden.load(Ordering::SeqCst), s.pet_minimized.load(Ordering::SeqCst)) {
                delay = Duration::from_millis(PAUSED_MS);
                continue;
            }
            // Until the frontend reports its rects, the whole window stays interactive.
            if lock(&s.hit_regions).is_none() {
                continue;
            }
            let Some(pet) = pet_window(&app) else { continue };
            let generation = s.pet_geometry_gen.load(Ordering::SeqCst);
            let stale = geometry.as_ref().is_none_or(|g| g.generation != generation || g.read_at.elapsed() >= GEOMETRY_MAX_AGE);
            if stale {
                geometry = read_geometry(&pet, generation);
            }
            let Some(g) = geometry.as_ref() else { continue };
            let Ok(cursor) = app.cursor_position() else { continue };
            let cursor = (cursor.x, cursor.y);
            let (ignore, id) = {
                let rects = lock(&s.hit_regions);
                let Some(rects) = rects.as_deref() else { continue };
                (!cursor_hits(cursor, g.origin, g.scale, rects), hovered_id(cursor, g.origin, g.scale, rects))
            };
            if ignoring != Some(ignore) && pet.set_ignore_cursor_events(ignore).is_ok() {
                ignoring = Some(ignore);
            }
            if id != hovered {
                let _ = app.emit_to(PET, "pet-pointer", &id);
                hovered = id;
            }
            let window = Area { x: g.origin.0, y: g.origin.1, w: g.size.0, h: g.size.1 };
            delay = poll_delay(cursor, window, g.scale);
            POLL_IS_SLOW.store(delay > Duration::from_millis(POLL_NEAR_MS), Ordering::SeqCst);
        }
    });
    let _ = POLLER.set(poller.thread().clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECT: HitRect = HitRect { x: 100.0, y: 400.0, w: 50.0, h: 40.0, id: None };

    #[test]
    fn hit_test_at_scale_one() {
        let origin = (1000, 200);
        assert!(cursor_hits((1120.0, 620.0), origin, 1.0, &[RECT]));
        assert!(!cursor_hits((1000.0, 200.0), origin, 1.0, &[RECT]));
        assert!(!cursor_hits((1120.0, 620.0), origin, 1.0, &[]));
    }

    #[test]
    fn hit_test_pads_by_six_px() {
        let origin = (0, 0);
        assert!(cursor_hits((94.0, 420.0), origin, 1.0, &[RECT]));
        assert!(!cursor_hits((93.0, 420.0), origin, 1.0, &[RECT]));
        assert!(cursor_hits((156.0, 446.0), origin, 1.0, &[RECT]));
        assert!(!cursor_hits((157.0, 420.0), origin, 1.0, &[RECT]));
        assert!(!cursor_hits((120.0, 447.0), origin, 1.0, &[RECT]));
    }

    #[test]
    fn hit_test_converts_physical_to_logical() {
        // At 150% the rect's logical (100..150, 400..440) is physical (150..225, 600..660) from the origin.
        let origin = (-1920, 100);
        assert!(cursor_hits((-1920.0 + 180.0, 100.0 + 630.0), origin, 1.5, &[RECT]));
        assert!(!cursor_hits((-1920.0 + 120.0, 100.0 + 630.0), origin, 1.5, &[RECT]));
        assert!(cursor_hits((-1920.0 + 145.0, 100.0 + 630.0), origin, 1.5, &[RECT]));
    }

    #[test]
    fn hit_test_any_of_several_rects() {
        let other = HitRect { x: 0.0, y: 0.0, w: 10.0, h: 10.0, id: None };
        assert!(cursor_hits((5.0, 5.0), (0, 0), 1.0, &[RECT, other.clone()]));
        assert!(cursor_hits((120.0, 420.0), (0, 0), 1.0, &[other, RECT]));
    }

    #[test]
    fn hit_test_survives_a_bad_scale() {
        assert!(cursor_hits((120.0, 420.0), (0, 0), 0.0, &[RECT]));
    }

    // ---- Adaptive cursor poll (design D17) ----

    /// A 380x600 window at 1000,300 (physical).
    const WIN: Area = Area { x: 1000, y: 300, w: 380, h: 600 };
    const NEAR: Duration = Duration::from_millis(POLL_NEAR_MS);
    const FAR: Duration = Duration::from_millis(POLL_FAR_MS);

    #[test]
    fn polls_fast_with_the_cursor_inside_the_window() {
        assert_eq!(poll_delay((1100.0, 500.0), WIN, 1.0), NEAR);
        assert_eq!(poll_delay((1000.0, 300.0), WIN, 1.0), NEAR, "on the corner");
        assert_eq!(poll_delay((1380.0, 900.0), WIN, 1.0), NEAR, "on the far corner");
    }

    #[test]
    fn polls_fast_within_64_logical_px_of_the_window() {
        assert_eq!(poll_delay((1000.0 - 64.0, 500.0), WIN, 1.0), NEAR, "left");
        assert_eq!(poll_delay((1380.0 + 64.0, 500.0), WIN, 1.0), NEAR, "right");
        assert_eq!(poll_delay((1100.0, 300.0 - 64.0), WIN, 1.0), NEAR, "above");
        assert_eq!(poll_delay((1100.0, 900.0 + 64.0), WIN, 1.0), NEAR, "below");
        // At 150%, 64 logical px is 96 physical.
        assert_eq!(poll_delay((1000.0 - 96.0, 500.0), WIN, 1.5), NEAR);
        assert!(poll_delay((1000.0 - 96.0, 500.0), WIN, 1.0) > NEAR, "the same 96 physical px is farther at 100%");
    }

    #[test]
    fn polls_slowly_far_from_the_window() {
        assert_eq!(poll_delay((0.0, 0.0), WIN, 1.0), FAR);
        assert_eq!(poll_delay((5000.0, 2000.0), WIN, 1.0), FAR);
        assert_eq!(poll_delay((-3000.0, 500.0), WIN, 2.0), FAR, "on another monitor");
    }

    #[test]
    fn slows_down_gradually_so_a_quick_approach_is_still_caught() {
        // Past 64 px the wait grows with the distance: the cursor can't cover the gap to the window
        // before the next poll unless it moves faster than about 2000 logical px/s.
        let at = |dx: f64| poll_delay((1000.0 - dx, 500.0), WIN, 1.0);
        let mut last = NEAR;
        for dx in [64.0, 65.0, 100.0, 200.0, 300.0, 400.0, 480.0, 500.0, 1000.0] {
            let d = at(dx);
            assert!(d >= last, "never faster farther away ({dx} px)");
            assert!(d == NEAR || d.as_secs_f64() * 2000.0 <= dx + 1.0, "{dx} px away must be polled again before 2000 px/s could cross it");
            last = d;
        }
        assert!(at(100.0) < Duration::from_millis(60), "100 px away polls about every 50 ms");
        assert_eq!(at(1000.0), FAR);
        // Diagonal distances count as distances, not per axis.
        assert!(poll_delay((1000.0 - 60.0, 300.0 - 60.0), WIN, 1.0) > NEAR, "85 px away on the diagonal");
    }

    #[test]
    fn poll_delay_survives_bad_input() {
        assert_eq!(poll_delay((1100.0, 500.0), WIN, 0.0), NEAR, "a bad scale is treated as 1");
        assert_eq!(poll_delay((0.0, 0.0), WIN, f64::NAN), FAR);
        assert_eq!(poll_delay((f64::NAN, 500.0), WIN, 1.0), NEAR, "unknown position: stay responsive");
    }

    #[test]
    fn the_poll_runs_only_while_the_pet_is_on_screen() {
        assert!(on_screen(false, false));
        assert!(!on_screen(true, false), "hidden (tray, Ctrl+Alt+P, hide for 1 hour)");
        assert!(!on_screen(false, true), "minimized");
        assert!(!on_screen(true, true));
    }

    #[test]
    fn a_zero_size_means_minimized() {
        // Windows reports a minimized window as resized to 0x0.
        assert!(minimized_size(0, 0));
        assert!(minimized_size(0, 600));
        assert!(!minimized_size(380, 600));
    }

    const WORK: Area = Area { x: 0, y: 0, w: 1920, h: 1032 };
    const SIZE: (i32, i32) = (380, 600);

    #[test]
    fn default_is_bottom_right_with_margin() {
        assert_eq!(default_position(WORK, SIZE, 16), (1920 - 380 - 16, 1032 - 600 - 16));
        let second = Area { x: 1920, y: 40, w: 2560, h: 1400 };
        assert_eq!(default_position(second, (570, 900), 24), (1920 + 2560 - 570 - 24, 40 + 1400 - 900 - 24));
    }

    #[test]
    fn default_on_a_short_screen_keeps_the_bottom_visible() {
        let short = Area { x: 0, y: 0, w: 1093, h: 566 };
        assert_eq!(default_position(short, SIZE, 16), (1093 - 380 - 16, 566 - 600));
    }

    #[test]
    fn clamps_saved_positions_into_the_nearest_work_area() {
        let areas = [WORK, Area { x: 1920, y: 0, w: 1280, h: 984 }];
        assert_eq!(clamp_position((100, 100), SIZE, &areas), Some((100, 100)));
        assert_eq!(clamp_position((1600, 500), SIZE, &areas), Some((1540, 432)));
        assert_eq!(clamp_position((2000, 800), SIZE, &areas), Some((2000, 384)));
        assert_eq!(clamp_position((-100, -50), SIZE, &areas), Some((0, 0)));
        assert_eq!(clamp_position((5000, 5000), SIZE, &areas), None);
        assert_eq!(clamp_position((10, 10), SIZE, &[]), None);
    }

    #[test]
    fn clamp_prefers_the_area_it_mostly_covers() {
        let areas = [WORK, Area { x: 1920, y: 0, w: 1280, h: 984 }];
        // 280 px on the right monitor, 100 px on the left: goes right.
        assert_eq!(clamp_position((1820, 100), SIZE, &areas), Some((1920, 100)));
    }

    #[test]
    fn clamp_into_handles_oversized_windows() {
        let tiny = Area { x: 10, y: 20, w: 300, h: 500 };
        assert_eq!(clamp_into(tiny, (0, 0), SIZE), (10 + (300 - 380) / 2, 20 + 500 - 600));
    }

    #[test]
    fn hit_rects_parse_from_the_frontend() {
        let r: Vec<HitRect> =
            serde_json::from_str(r#"[{"x":1,"y":2.5,"w":3,"h":4},{"x":0,"y":0,"w":1,"h":1,"id":"pet"}]"#).unwrap();
        assert_eq!(r[0], HitRect { x: 1.0, y: 2.5, w: 3.0, h: 4.0, id: None });
        assert_eq!(r[1].id.as_deref(), Some("pet"));
    }

    #[test]
    fn hovered_id_names_the_rect_under_the_cursor() {
        let card = HitRect { x: 0.0, y: 0.0, w: 80.0, h: 80.0, id: None };
        let pet = HitRect { x: 100.0, y: 400.0, w: 50.0, h: 40.0, id: Some("pet".into()) };
        let bar = HitRect { x: 100.0, y: 450.0, w: 50.0, h: 20.0, id: Some("bar".into()) };
        let rects = [card, pet, bar];
        assert_eq!(hovered_id((120.0, 420.0), (0, 0), 1.0, &rects).as_deref(), Some("pet"));
        assert_eq!(hovered_id((120.0, 460.0), (0, 0), 1.0, &rects).as_deref(), Some("bar"));
        assert_eq!(hovered_id((40.0, 40.0), (0, 0), 1.0, &rects), None);
        assert_eq!(hovered_id((300.0, 300.0), (0, 0), 1.0, &rects), None);
    }

    // ---- Design D9: sprite/stage placement ----

    const D9_AREA: Area = Area { x: 0, y: 0, w: 1920, h: 1032 };
    const D9_WINDOW: (i32, i32) = (380, 600);
    const D9_PET_SCALE: f64 = 0.6;

    #[test]
    fn placement_serializes_for_the_frontend() {
        let p = Placement { cards_below: true, shift_x: -12.5, stage_room: 200.0 };
        assert_eq!(
            serde_json::to_value(p).unwrap(),
            serde_json::json!({"cardsBelow": true, "shiftX": -12.5, "stageRoom": 200.0})
        );
    }

    #[test]
    fn flip_threshold_has_hysteresis() {
        // In the dead zone between the two thresholds, whichever state it was in wins.
        assert!(!next_cards_below(false, 350.0), "stays above: it was above and there's still room");
        assert!(next_cards_below(true, 350.0), "stays below: it was below and room hasn't clearly recovered");
        // Entering "below" needs room above under the (lower) enter threshold.
        assert!(!next_cards_below(false, FLIP_ENTER));
        assert!(next_cards_below(false, FLIP_ENTER - 0.1));
        // Leaving "below" needs room above clearing the (higher) exit threshold.
        assert!(next_cards_below(true, FLIP_EXIT));
        assert!(!next_cards_below(true, FLIP_EXIT + 0.1));
    }

    #[test]
    fn sprite_offset_is_centered_and_flips_between_top_and_bottom_of_the_window() {
        let (sw, sh) = sprite_size(D9_PET_SCALE);
        assert_eq!((sw, sh), (192.0 * 0.6, 208.0 * 0.6));
        let window = (380.0, 600.0);
        let (ox_below, oy_below) = sprite_offset(true, D9_PET_SCALE, window);
        assert_eq!((ox_below, oy_below), ((380.0 - sw) / 2.0, OVERLAY_PAD_TOP));
        let (ox_above, oy_above) = sprite_offset(false, D9_PET_SCALE, window);
        assert_eq!(ox_above, ox_below, "always horizontally centered regardless of the flip");
        assert_eq!(oy_above, 600.0 - OVERLAY_PAD_BOTTOM - BAR_HEIGHT - DOCK_GAP - sh);
    }

    #[test]
    fn sprite_sits_flush_in_every_corner_with_the_window_free_to_go_off_screen() {
        let sprite_size = sprite_size_physical(D9_PET_SCALE, 1.0);
        let corners = [
            (D9_AREA.x, D9_AREA.y),
            (D9_AREA.x + D9_AREA.w - sprite_size.0, D9_AREA.y),
            (D9_AREA.x, D9_AREA.y + D9_AREA.h - sprite_size.1),
            (D9_AREA.x + D9_AREA.w - sprite_size.0, D9_AREA.y + D9_AREA.h - sprite_size.1),
        ];
        for anchor in corners {
            // Already flush at the corner: the sprite's own clamp must leave it exactly there.
            assert_eq!(clamp_position(anchor, sprite_size, &[D9_AREA]), Some(anchor), "{anchor:?}");
            let (window_pos, placement) = place_sprite(anchor, D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
            let (ox, oy) = sprite_offset(placement.cards_below, D9_PET_SCALE, (380.0, 600.0));
            let sprite_at = (window_pos.0 + ox.round() as i32, window_pos.1 + oy.round() as i32);
            assert_eq!(sprite_at, anchor, "the sprite must land exactly at its clamped corner: {anchor:?}");
        }
    }

    #[test]
    fn top_edge_flips_cards_below_and_bottom_edge_does_not() {
        let sprite_size = sprite_size_physical(D9_PET_SCALE, 1.0);
        let (_, top) = place_sprite((900, 0), D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(top.cards_below, "no room above at the top edge");
        let bottom_anchor = (900, D9_AREA.h - sprite_size.1);
        let (_, bottom) = place_sprite(bottom_anchor, D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(!bottom.cards_below, "plenty of room above at the bottom edge");
    }

    #[test]
    fn shifts_the_stage_away_from_the_left_and_right_edges_only() {
        let sprite_size = sprite_size_physical(D9_PET_SCALE, 1.0);
        let (_, left) = place_sprite((0, 500), D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(left.shift_x > 0.0, "the window hangs off the left edge, so the stage shifts right");
        let (_, right) = place_sprite((D9_AREA.w - sprite_size.0, 500), D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(right.shift_x < 0.0, "the window hangs off the right edge, so the stage shifts left");
        // Not perfectly symmetric: the sprite width rounds to 115 physical px from 115.2 logical.
        assert_eq!(left.shift_x, 132.0);
        assert_eq!(right.shift_x, -133.0);
        let (_, middle) = place_sprite((900, 500), D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert_eq!(middle.shift_x, 0.0, "comfortably inside both edges: no shift needed");
    }

    #[test]
    fn stage_shift_centers_in_a_work_area_narrower_than_the_window() {
        let area = Area { x: 500, y: 0, w: 300, h: 1032 }; // narrower than the 380-wide window
        let (window_pos, placement) = place_sprite((550, 900), area, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        let centered_window_x = area.x + (area.w - D9_WINDOW.0) / 2;
        assert_eq!(centered_window_x, 460);
        assert_eq!(placement.shift_x, f64::from(centered_window_x - window_pos.0));
        assert_eq!(placement.shift_x, 42.0);
    }

    #[test]
    fn place_sprite_handles_a_15x_scale_factor() {
        let area = Area { x: 0, y: 0, w: 2880, h: 1550 };
        let window_physical = (570, 900); // 380x600 logical at 1.5x
        let (window_pos, placement) = place_sprite((100, 50), area, 1.5, window_physical, D9_PET_SCALE, false);
        assert!(placement.cards_below, "only ~33 logical px above at 1.5x: not enough room");
        assert_eq!(window_pos, (-99, 26));
        assert_eq!(placement.shift_x, 66.0);
    }

    #[test]
    fn migrates_a_v02_window_position_to_the_sprite_anchor() {
        // v0.2 saved the window's top-left; the un-flipped offset at pet_scale 0.6 is (132, 423).
        let anchor = migrate_saved_position((1000, 300), 1.0, D9_PET_SCALE, D9_WINDOW);
        assert_eq!(anchor, (1000 + 132, 300 + 423));
    }

    #[test]
    fn migrated_position_lands_the_sprite_at_the_same_screen_spot_as_before() {
        let window_pos = (1200, 200);
        let anchor = migrate_saved_position(window_pos, 1.0, D9_PET_SCALE, D9_WINDOW);
        let (new_window_pos, placement) = place_sprite(anchor, D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(!placement.cards_below);
        assert_eq!(new_window_pos, window_pos, "gate G2.2: the same visual spot as the old window-anchored save");
    }

    // ---- stageRoom: how much of the stage is actually on screen (review round 2) ----

    #[test]
    fn stage_room_is_capped_by_the_work_area_top_when_unflipped() {
        // Room above is 350: inside the dead zone, so cards stay above (unflipped). The stage's
        // actual bottom-up room on screen is that minus the gap to the dock, not its full height.
        let (_, placement) = place_sprite((900, 350), D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(!placement.cards_below);
        assert_eq!(placement.stage_room, 350.0 - OVERLAY_GAP);
        assert!(placement.stage_room < stage_natural_height(D9_PET_SCALE, 600.0), "must be less than the full height");
    }

    #[test]
    fn stage_room_is_capped_by_the_work_area_bottom_when_flipped() {
        let small = Area { x: 0, y: 0, w: 1920, h: 300 };
        let (_, placement) = place_sprite((900, 0), small, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(placement.cards_below);
        let sh = sprite_size(D9_PET_SCALE).1;
        let expected = 300.0 - (sh + DOCK_GAP + BAR_HEIGHT + OVERLAY_GAP);
        assert_eq!(placement.stage_room, expected);
    }

    #[test]
    fn stage_room_is_the_natural_height_with_room_to_spare() {
        let (_, placement) = place_sprite((900, 500), D9_AREA, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(!placement.cards_below);
        assert_eq!(placement.stage_room, stage_natural_height(D9_PET_SCALE, 600.0));
    }

    #[test]
    fn stage_room_never_goes_negative() {
        // A work area barely bigger than the dock: flipped, the dock alone almost fills it, so the
        // stage's room below would go negative before clamping.
        let tiny = Area { x: 0, y: 0, w: 1920, h: 40 };
        let (_, placement) = place_sprite((900, 0), tiny, 1.0, D9_WINDOW, D9_PET_SCALE, false);
        assert!(placement.cards_below, "no room above a 40px-tall area either, so it flips below");
        assert_eq!(placement.stage_room, 0.0);
    }

    // ---- resolve_sprite_anchor: off-screen recovery (gate G3.5, review round 2) ----

    #[test]
    fn resolve_sprite_anchor_keeps_an_on_screen_saved_value_clamped() {
        let sprite_size = sprite_size_physical(D9_PET_SCALE, 1.0);
        let (anchor, fell_back) = resolve_sprite_anchor(Some((100, 100)), sprite_size, &[D9_AREA], (1700, 900));
        assert_eq!(anchor, (100, 100));
        assert!(!fell_back);
    }

    #[test]
    fn resolve_sprite_anchor_falls_back_to_the_default_when_the_saved_monitor_is_gone() {
        let sprite_size = sprite_size_physical(D9_PET_SCALE, 1.0);
        // The saved spot was on a second monitor that doesn't exist anymore.
        let vanished = (2500, 300);
        let default = (1700, 900);
        let (anchor, fell_back) = resolve_sprite_anchor(Some(vanished), sprite_size, &[D9_AREA], default);
        assert_eq!(anchor, default);
        assert!(fell_back);
    }

    #[test]
    fn resolve_sprite_anchor_uses_the_default_when_nothing_is_saved() {
        let sprite_size = sprite_size_physical(D9_PET_SCALE, 1.0);
        let default = (1700, 900);
        let (anchor, fell_back) = resolve_sprite_anchor(None, sprite_size, &[D9_AREA], default);
        assert_eq!(anchor, default);
        assert!(fell_back);
    }
}
