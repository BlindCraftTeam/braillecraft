//! Screen reader support settings and accessible names.
//!
//! The editing area is the system's own multi-line text control, so every screen reader reads it through the
//! system text interfaces. Global, Internationalization keeps the Support UI Automation choice. On Linux
//! wxWidgets passes no accessible names to the accessibility interface, so the document window's controls are
//! named here; on Windows and macOS the control's accessibility label does that.

use std::ffi::c_void;

#[derive(Clone, Debug)]
pub struct ReaderSettings {
    /// Global, Internationalization: Support UI Automation.
    pub uia: bool,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        ReaderSettings { uia: true }
    }
}

/// Gives a control its accessible name on Linux (see the module note).
pub fn set_name(widget: *mut c_void, name: &str) {
    imp::set_name(widget, name);
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
mod imp {
    use std::ffi::c_void;

    pub fn set_name(_widget: *mut c_void, _name: &str) {}
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use std::ffi::{c_char, c_void, CString};

    #[link(name = "gtk-3")]
    extern "C" {
        fn gtk_widget_get_accessible(widget: *mut c_void) -> *mut c_void;
        fn gtk_bin_get_type() -> usize;
        fn gtk_bin_get_child(bin: *mut c_void) -> *mut c_void;
    }

    #[link(name = "atk-1.0")]
    extern "C" {
        fn atk_object_set_name(accessible: *mut c_void, name: *const c_char);
    }

    #[link(name = "gobject-2.0")]
    extern "C" {
        fn g_type_check_instance_is_a(instance: *mut c_void, itype: usize) -> i32;
    }

    /// Names the widget and, for a container such as the scrolled window around a multi-line text view, the widget
    /// inside it, which is the one that takes the focus.
    pub fn set_name(widget: *mut c_void, name: &str) {
        if widget.is_null() {
            return;
        }
        let Ok(c_name) = CString::new(name) else { return };
        unsafe {
            let mut targets = vec![widget];
            if g_type_check_instance_is_a(widget, gtk_bin_get_type()) != 0 {
                let child = gtk_bin_get_child(widget);
                if !child.is_null() {
                    targets.push(child);
                }
            }
            for w in targets {
                let accessible = gtk_widget_get_accessible(w);
                if !accessible.is_null() {
                    atk_object_set_name(accessible, c_name.as_ptr());
                }
            }
        }
    }
}
