//! The pet overlay window: click-through hit testing and on-screen placement.

use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, WebviewWindow};

use crate::state::AppState;

pub const PET: &str = "pet";
pub const HIT_PADDING: f64 = 6.0;
pub const POLL_MS: u64 = 33;
/// Gap between the pet window and the work-area edges at the default position, in logical px.
pub const EDGE_MARGIN: f64 = 16.0;

/// An interactive rectangle reported by the frontend, in window-relative CSS px.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct HitRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A screen rectangle in physical px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Whether a physical cursor position falls inside any padded rect of a window at `origin` with `scale`.
pub fn cursor_hits(cursor: (f64, f64), origin: (i32, i32), scale: f64, rects: &[HitRect]) -> bool {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let x = (cursor.0 - f64::from(origin.0)) / scale;
    let y = (cursor.1 - f64::from(origin.1)) / scale;
    rects.iter().any(|r| {
        x >= r.x - HIT_PADDING && x <= r.x + r.w + HIT_PADDING && y >= r.y - HIT_PADDING && y <= r.y + r.h + HIT_PADDING
    })
}

/// Moves a window of `size` fully inside `area`. A window taller than the area keeps its bottom edge
/// (where the pet sits) on screen; one wider than the area is centered.
pub fn clamp_into(area: Area, pos: (i32, i32), size: (i32, i32)) -> (i32, i32) {
    let x = if size.0 > area.w {
        area.x + (area.w - size.0) / 2
    } else {
        pos.0.clamp(area.x, area.x + area.w - size.0)
    };
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

/// Clamps a window into the work area it overlaps most. None when it is on no work area at all.
pub fn clamp_position(pos: (i32, i32), size: (i32, i32), areas: &[Area]) -> Option<(i32, i32)> {
    let (best, amount) = areas.iter().map(|a| (*a, overlap(*a, pos, size))).max_by_key(|(_, o)| *o)?;
    (amount > 0).then(|| clamp_into(best, pos, size))
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

fn default_for(pet: &WebviewWindow) -> Option<(i32, i32)> {
    let monitor = pet
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| pet.available_monitors().ok().and_then(|m| m.into_iter().next()))?;
    let margin = (EDGE_MARGIN * monitor.scale_factor()).round() as i32;
    Some(default_position(work_area(&monitor), outer_size(pet), margin))
}

fn set_pos(pet: &WebviewWindow, pos: (i32, i32)) {
    let _ = pet.set_position(PhysicalPosition::new(pos.0, pos.1));
}

fn remember(app: &AppHandle, pos: Option<(i32, i32)>) {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().pet_position = pos;
    s.save_config();
}

/// Startup placement: the saved spot clamped into a visible work area, else the default.
pub fn place_pet(app: &AppHandle) {
    let Some(pet) = pet_window(app) else { return };
    let saved = app.state::<AppState>().config.lock().unwrap().pet_position;
    let pos = saved.and_then(|p| clamp_position(p, outer_size(&pet), &work_areas(&pet))).or_else(|| default_for(&pet));
    if let Some(pos) = pos {
        set_pos(&pet, pos);
    }
}

pub fn reset_pet_position(app: &AppHandle) {
    let Some(pet) = pet_window(app) else { return };
    if let Some(pos) = default_for(&pet) {
        set_pos(&pet, pos);
    }
    remember(app, None);
}

/// Nudges the pet by logical px, keeping it inside a work area.
pub fn move_pet_by(app: &AppHandle, dx: f64, dy: f64) {
    let Some(pet) = pet_window(app) else { return };
    let (Ok(pos), Ok(scale)) = (pet.outer_position(), pet.scale_factor()) else { return };
    let target = (pos.x + (dx * scale).round() as i32, pos.y + (dy * scale).round() as i32);
    let pos = clamp_position(target, outer_size(&pet), &work_areas(&pet)).unwrap_or(target);
    set_pos(&pet, pos);
    remember(app, Some(pos));
}

/// Polls the cursor and lets clicks fall through the pet window except over the reported rects.
pub fn start_click_through(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ignoring: Option<bool> = None;
        loop {
            std::thread::sleep(Duration::from_millis(POLL_MS));
            // Until the frontend reports its rects, the whole window stays interactive.
            let Some(rects) = app.state::<AppState>().hit_regions.lock().unwrap().clone() else { continue };
            let Some(pet) = pet_window(&app) else { continue };
            let Ok(cursor) = app.cursor_position() else { continue };
            let Ok(origin) = pet.inner_position().or_else(|_| pet.outer_position()) else { continue };
            let Ok(scale) = pet.scale_factor() else { continue };
            let ignore = !cursor_hits((cursor.x, cursor.y), (origin.x, origin.y), scale, &rects);
            if ignoring != Some(ignore) && pet.set_ignore_cursor_events(ignore).is_ok() {
                ignoring = Some(ignore);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECT: HitRect = HitRect { x: 100.0, y: 400.0, w: 50.0, h: 40.0 };

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
        let other = HitRect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
        assert!(cursor_hits((5.0, 5.0), (0, 0), 1.0, &[RECT, other]));
        assert!(cursor_hits((120.0, 420.0), (0, 0), 1.0, &[other, RECT]));
    }

    #[test]
    fn hit_test_survives_a_bad_scale() {
        assert!(cursor_hits((120.0, 420.0), (0, 0), 0.0, &[RECT]));
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
        let r: Vec<HitRect> = serde_json::from_str(r#"[{"x":1,"y":2.5,"w":3,"h":4}]"#).unwrap();
        assert_eq!(r, vec![HitRect { x: 1.0, y: 2.5, w: 3.0, h: 4.0 }]);
    }
}
