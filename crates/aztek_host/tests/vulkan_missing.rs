//! A machine with no Vulkan driver (#315): the viewport fails once, says why once, and hands the reason to the panels.
//! Needs an X display (run it under xvfb-run); without one it checks only what it can.

use std::ffi::{c_void, CStr};

use aztek_host::{aztek_free, aztek_take_log, aztek_view_create, aztek_view_error};

const AZTEK_XCB: i32 = 1;

fn take_errors() -> Vec<String> {
    let p = aztek_take_log();
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    v.as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn view_error() -> Option<String> {
    let p = aztek_view_error();
    if p.is_null() {
        return None;
    }
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    Some(s)
}

/// An xcb connection to $DISPLAY through the system's libxcb, or None. Unix only: libc, which opens it, is a Unix
/// dependency, so on Windows the test checks only what needs no display.
#[cfg(not(unix))]
fn xcb_connection() -> Option<*mut c_void> {
    None
}

/// An xcb connection to $DISPLAY through the system's libxcb, or None.
#[cfg(unix)]
fn xcb_connection() -> Option<*mut c_void> {
    use std::ffi::c_char;
    std::env::var_os("DISPLAY")?;
    unsafe {
        let lib = libc::dlopen(c"libxcb.so.1".as_ptr(), libc::RTLD_NOW);
        if lib.is_null() {
            return None;
        }
        let f = libc::dlsym(lib, c"xcb_connect".as_ptr());
        if f.is_null() {
            return None;
        }
        let connect: extern "C" fn(*const c_char, *mut i32) -> *mut c_void = std::mem::transmute(f);
        let c = connect(std::ptr::null(), std::ptr::null_mut());
        (!c.is_null()).then_some(c)
    }
}

#[test]
fn no_vulkan_driver_fails_once_and_tells_the_panels_why() {
    // #315: a driver list naming nothing is what a machine with no Vulkan driver looks like to the loader.
    std::env::set_var("VK_DRIVER_FILES", "/nonexistent/aztek_no_driver.json");
    std::env::set_var("VK_ICD_FILENAMES", "/nonexistent/aztek_no_driver.json");
    assert_eq!(view_error(), None, "nothing has failed before a view is asked for");
    let cam = c"persp";
    // An unknown window system is not a Vulkan failure.
    assert_eq!(aztek_view_create(99, std::ptr::null_mut(), 1, std::ptr::null_mut(), 64, 64, 1.0, cam.as_ptr()), 0);
    assert_eq!(view_error(), None);
    take_errors();

    let Some(conn) = xcb_connection() else { return };
    // The instance fails before the window is used, so any window id does.
    assert_eq!(aztek_view_create(AZTEK_XCB, conn, 1, std::ptr::null_mut(), 64, 64, 1.0, cam.as_ptr()), 0);
    let errors = take_errors();
    assert_eq!(errors.iter().filter(|e| e.contains("Vulkan")).count(), 1, "{errors:?}");
    let why = view_error().expect("the reason is kept for the panels");
    assert!(!why.is_empty());

    // Every other view, and every repaint, is quiet: the history doesn't fill with the same error.
    for _ in 0..5 {
        assert_eq!(aztek_view_create(AZTEK_XCB, conn, 1, std::ptr::null_mut(), 64, 64, 1.0, cam.as_ptr()), 0);
    }
    assert!(take_errors().is_empty());
    assert_eq!(view_error().as_deref(), Some(why.as_str()));
}
