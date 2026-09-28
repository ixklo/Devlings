//! Keeps the pet in Windows' always-on-top band (design v1.2 D23.6). Windows moves topmost windows below an app that
//! goes full screen, so they don't cover it, and doesn't always lift them back afterwards: the pet window keeps its
//! topmost flag but sits under ordinary windows, invisible. `overlay::rescue_pet_soon`'s periodic check asks
//! `pet_demoted` and puts the window back on top, except while a full-screen app is in front of it.
//! Only Windows reads the stacking order, so elsewhere the check's types and logic are used by the tests alone.
#![cfg_attr(not(any(windows, test)), allow(dead_code))]

/// Left, top, right, bottom in physical pixels.
pub type Rect = (i32, i32, i32, i32);

/// The pet window's place in the stacking order: its own topmost flag, the windows above it (nearest first) and its
/// monitor's bounds.
#[cfg(windows)]
pub struct Stack {
    pub own_topmost: bool,
    pub above: Vec<Above>,
    pub monitor: Rect,
}

/// A window above the pet in Windows' stacking order, as far as the check needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Above {
    pub visible: bool,
    pub cloaked: bool,
    pub topmost: bool,
    pub rect: Rect,
}

fn covers(rect: Rect, monitor: Rect) -> bool {
    rect.0 <= monitor.0 && rect.1 <= monitor.1 && rect.2 >= monitor.2 && rect.3 >= monitor.3
}

/// Whether the pet has dropped out of the always-on-top band: it lost its topmost flag, or an ordinary (not topmost)
/// window that's visible sits above it. A window above it that covers its whole monitor is a full-screen app, which
/// the pet is meant to stay behind, so then it's left alone.
pub fn pet_demoted(own_topmost: bool, above: &[Above], monitor: Rect) -> bool {
    let shown = |w: &&Above| w.visible && !w.cloaked;
    if above.iter().filter(shown).any(|w| !w.topmost && covers(w.rect, monitor)) {
        return false;
    }
    !own_topmost || above.iter().filter(shown).any(|w| !w.topmost)
}

/// Reads the pet window's place in the stacking order.
#[cfg(windows)]
pub fn stack_of(hwnd: *mut std::ffi::c_void) -> Option<Stack> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindow, GetWindowLongW, GetWindowRect, IsWindowVisible, GWL_EXSTYLE, GW_HWNDPREV, WS_EX_TOPMOST,
    };
    let hwnd = HWND(hwnd);
    let topmost = |h: HWND| (unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0) != 0;
    // SAFETY: plain Win32 queries on window handles; a handle that went away just makes a call fail.
    let monitor = unsafe {
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info).as_bool() {
            return None;
        }
        let r = info.rcMonitor;
        (r.left, r.top, r.right, r.bottom)
    };
    let mut above = Vec::new();
    let mut h = unsafe { GetWindow(hwnd, GW_HWNDPREV) }.ok();
    // The stacking order above the pet is short in practice; the cap guards against a list that changes mid-walk.
    while let Some(w) = h.filter(|w| !w.is_invalid()) {
        if above.len() >= 1000 {
            break;
        }
        let mut rect = RECT::default();
        let mut cloaked = 0u32;
        unsafe {
            let _ = GetWindowRect(w, &mut rect);
            let _ = DwmGetWindowAttribute(w, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), std::mem::size_of::<u32>() as u32);
        }
        above.push(Above {
            visible: unsafe { IsWindowVisible(w) }.as_bool(),
            cloaked: cloaked != 0,
            topmost: topmost(w),
            rect: (rect.left, rect.top, rect.right, rect.bottom),
        });
        h = unsafe { GetWindow(w, GW_HWNDPREV) }.ok();
    }
    Some(Stack { own_topmost: topmost(hwnd), above, monitor })
}

/// Puts the pet window back at the top of the always-on-top band, without activating it.
#[cfg(windows)]
pub fn raise(hwnd: *mut std::ffi::c_void) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
    };
    // SAFETY: SetWindowPos on our own window handle; failure only means it stays where it is.
    let _ = unsafe { SetWindowPos(HWND(hwnd), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER) };
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = (0, 0, 2880, 1620);
    const fn win(visible: bool, cloaked: bool, topmost: bool, rect: Rect) -> Above {
        Above { visible, cloaked, topmost, rect }
    }
    const TASKBAR: Above = win(true, false, true, (0, 1524, 2880, 1620));
    const MAXIMIZED: Above = win(true, false, false, (-13, -13, 2893, 1537));
    const PHOTOS: Above = win(true, false, false, (98, 98, 2258, 1204));
    const FULL_SCREEN: Above = win(true, false, false, (0, 0, 2880, 1620));

    #[test]
    fn on_top_when_only_topmost_windows_are_above() {
        assert!(!pet_demoted(true, &[], MONITOR));
        assert!(!pet_demoted(true, &[TASKBAR, win(true, false, true, (0, 0, 880, 1620))], MONITOR));
    }

    #[test]
    fn demoted_when_an_ordinary_window_is_above_it() {
        // What the owner's PC showed: Edge, VS Code and Photos stacked above the topmost pet.
        assert!(pet_demoted(true, &[TASKBAR, MAXIMIZED, PHOTOS], MONITOR));
        assert!(pet_demoted(true, &[PHOTOS], MONITOR));
    }

    #[test]
    fn demoted_when_it_lost_its_topmost_flag() {
        assert!(pet_demoted(false, &[TASKBAR], MONITOR));
    }

    #[test]
    fn hidden_or_cloaked_windows_above_it_dont_count() {
        assert!(!pet_demoted(true, &[win(false, false, false, (0, 0, 100, 100)), win(true, true, false, (0, 0, 500, 500))], MONITOR));
    }

    #[test]
    fn stays_behind_a_full_screen_app() {
        assert!(!pet_demoted(true, &[FULL_SCREEN], MONITOR));
        assert!(!pet_demoted(true, &[TASKBAR, FULL_SCREEN, PHOTOS], MONITOR));
        // A maximized window leaves the taskbar showing: not full screen.
        assert!(pet_demoted(true, &[MAXIMIZED], MONITOR));
    }
}
