//! Multi-monitor aware window position manager.
//!
//! On Windows, virtual desktop coordinates can span across multiple displays
//! with different DPI scaling factors. Win32 physical pixels are unambiguous
//! across all monitors. This module directly queries and sets physical coordinates
//! via Win32 API and verifies monitor bounds to prevent windows from opening
//! off-screen (e.g. if a secondary monitor was disconnected).

#[cfg(target_os = "windows")]
pub mod platform {
    use std::sync::atomic::{AtomicIsize, Ordering};

    static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

    #[repr(C)]
    #[derive(Copy, Clone, Default, Debug)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    #[repr(C)]
    pub struct MONITORINFO {
        pub cb_size: u32,
        pub rc_monitor: RECT,
        pub rc_work: RECT,
        pub dw_flags: u32,
    }

    type BOOL = i32;
    type HWND = isize;
    type LPARAM = isize;
    type HMONITOR = isize;
    type WNDENUMPROC = unsafe extern "system" fn(HWND, LPARAM) -> BOOL;

    extern "system" {
        fn GetCurrentProcessId() -> u32;
        fn GetWindowThreadProcessId(hwnd: HWND, lpdwProcessId: *mut u32) -> u32;
        fn EnumWindows(lpEnumFunc: WNDENUMPROC, lParam: LPARAM) -> BOOL;
        fn GetWindowTextW(hwnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
        fn GetWindowRect(hwnd: HWND, lpRect: *mut RECT) -> BOOL;
        fn SetWindowPos(
            hwnd: HWND,
            hwnd_insert_after: HWND,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> BOOL;
        fn MonitorFromRect(lprc: *const RECT, dw_flags: u32) -> HMONITOR;
        fn GetMonitorInfoW(h_monitor: HMONITOR, lpmi: *mut MONITORINFO) -> BOOL;
    }

    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const MONITOR_DEFAULTTONULL: u32 = 0;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let my_pid = lparam as u32;
        if pid == my_pid {
            let mut buf = [0u16; 64];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            if len > 0 {
                let title = String::from_utf16_lossy(&buf[..len as usize]);
                if title == "Bitcoin Ticker" {
                    MAIN_HWND.store(hwnd, Ordering::SeqCst);
                    return 0; // stop enumeration
                }
            }
        }
        1
    }

    pub fn get_main_hwnd() -> isize {
        let existing = MAIN_HWND.load(Ordering::SeqCst);
        if existing != 0 {
            return existing;
        }
        unsafe {
            let my_pid = GetCurrentProcessId();
            EnumWindows(enum_window_callback, my_pid as LPARAM);
        }
        MAIN_HWND.load(Ordering::SeqCst)
    }

    /// Read the window's physical coordinates directly from the OS.
    pub fn get_physical_window_pos() -> Option<[f32; 2]> {
        let hwnd = get_main_hwnd();
        if hwnd == 0 {
            return None;
        }
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                return Some([rect.left as f32, rect.top as f32]);
            }
        }
        None
    }

    /// Set window to physical coordinates, clamped safely within an active monitor's work area.
    pub fn set_physical_window_pos(pos: [f32; 2], width: f32, height: f32) {
        let hwnd = get_main_hwnd();
        if hwnd == 0 {
            return;
        }
        let clamped = clamp_to_screen(pos[0], pos[1], width, height);
        unsafe {
            SetWindowPos(
                hwnd,
                0,
                clamped[0] as i32,
                clamped[1] as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    /// Validates whether the given rectangle intersects any currently connected monitor.
    /// If so, clamps it within that monitor's work area.
    /// If not (e.g. monitor unplugged), moves it safely to the nearest/primary monitor.
    pub fn clamp_to_screen(x: f32, y: f32, width: f32, height: f32) -> [f32; 2] {
        let rect = RECT {
            left: x as i32,
            top: y as i32,
            right: (x + width) as i32,
            bottom: (y + height) as i32,
        };

        unsafe {
            let mut hmon = MonitorFromRect(&rect, MONITOR_DEFAULTTONULL);
            if hmon == 0 {
                hmon = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
            }
            if hmon != 0 {
                let mut mi: MONITORINFO = std::mem::zeroed();
                mi.cb_size = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(hmon, &mut mi) != 0 {
                    let work = mi.rc_work;
                    let clamped_x = (x as i32)
                        .max(work.left)
                        .min(work.right - (width as i32).max(100));
                    let clamped_y = (y as i32)
                        .max(work.top)
                        .min(work.bottom - (height as i32).max(40));
                    return [clamped_x as f32, clamped_y as f32];
                }
            }
        }
        [100.0, 100.0]
    }
}

#[cfg(not(target_os = "windows"))]
pub mod platform {
    pub fn get_physical_window_pos() -> Option<[f32; 2]> {
        None
    }
    pub fn set_physical_window_pos(_pos: [f32; 2], _width: f32, _height: f32) {}
    pub fn clamp_to_screen(x: f32, y: f32, _width: f32, _height: f32) -> [f32; 2] {
        [x, y]
    }
}

#[cfg(test)]
mod tests {
    use super::platform::*;

    #[test]
    fn clamp_test() {
        let pos = clamp_to_screen(100.0, 100.0, 520.0, 100.0);
        assert!(pos[0] >= 0.0);
        assert!(pos[1] >= 0.0);
    }
}
