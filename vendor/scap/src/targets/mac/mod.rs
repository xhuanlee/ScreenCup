use cidre::{cg, ns, sc};
use cocoa::appkit::{NSApp, NSScreen};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSRect, NSString, NSUInteger};
use futures::executor::block_on;
use objc::{msg_send, sel, sel_impl};

use crate::engine::mac::ext::DirectDisplayIdExt;

use super::{Display, Target};

fn get_display_name(display_id: cg::DirectDisplayId) -> String {
    unsafe {
        // Get all screens
        let screens: id = NSScreen::screens(nil);
        let count: u64 = msg_send![screens, count];

        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: i];
            let device_description: id = msg_send![screen, deviceDescription];
            let display_id_number: id = msg_send![device_description, objectForKey: NSString::alloc(nil).init_str("NSScreenNumber")];
            let display_id_number: u32 = msg_send![display_id_number, unsignedIntValue];

            if display_id_number == display_id.0 {
                let localized_name: id = msg_send![screen, localizedName];
                let name: *const i8 = msg_send![localized_name, UTF8String];
                return std::ffi::CStr::from_ptr(name)
                    .to_string_lossy()
                    .into_owned();
            }
        }

        format!("Unknown Display {}", display_id.0)
    }
}

pub fn get_all_targets() -> Vec<Target> {
    let mut targets: Vec<Target> = Vec::new();

    let content = block_on(sc::ShareableContent::current()).unwrap();

    // Add displays to targets
    for display in content.displays().iter() {
        let id = display.display_id();

        let title = get_display_name(id);

        let target = Target::Display(super::Display {
            id: id.0,
            title,
            raw_handle: id,
        });

        targets.push(target);
    }

    // Add windows to targets
    for window in content.windows().iter() {
        let id = window.id();
        let title = window
            .title()
            // on intel chips we can have Some but also a null pointer for some reason
            .filter(|v| !unsafe { v.utf8_chars_ar().is_null() });

        let target = Target::Window(super::Window {
            id,
            title: title.map(|v| v.to_string()).unwrap_or_default(),
            raw_handle: id,
        });
        targets.push(target);
    }

    targets
}

pub fn get_main_display() -> Display {
    let id = cg::direct_display::Id::main();
    let title = get_display_name(id);

    Display {
        id: id.0,
        title,
        raw_handle: id,
    }
}

/// Patched: `NSApp -windowWithWindowNumber:` only resolves windows owned by
/// the current process and returns `nil` for other apps' windows, which made
/// the original `msg_send![ns_window, frame]` dereference null and abort the
/// process. Foreign windows fall back to the main display's backing scale.
pub fn get_scale_factor(target: &Target) -> f64 {
    match target {
        Target::Window(window) => unsafe {
            let cg_win_id = window.raw_handle;
            let ns_app: id = NSApp();
            let ns_window: id = msg_send![ns_app, windowWithWindowNumber: cg_win_id as NSUInteger];
            if ns_window == nil {
                return main_display_scale();
            }
            let scale_factor: f64 = msg_send![ns_window, backingScaleFactor];
            scale_factor
        },
        Target::Display(display) => {
            let mode = display.raw_handle.display_mode().unwrap();
            (mode.pixel_width() / mode.width()) as f64
        }
    }
}

/// Backing scale of the main display, used as a fallback for foreign windows.
fn main_display_scale() -> f64 {
    unsafe {
        let main = cg::direct_display::Id::main();
        match main.display_mode() {
            Some(mode) => (mode.pixel_width() / mode.width()) as f64,
            None => 1.0,
        }
    }
}

/// Patched: query window geometry through the cross-process CGWindowList API
/// instead of `NSApp -windowWithWindowNumber:` (see `get_scale_factor`).
pub fn get_target_dimensions(target: &Target) -> (u64, u64) {
    match target {
        Target::Window(window) => window_dimensions_from_cg_list(window.raw_handle),
        Target::Display(display) => {
            let mode = display.raw_handle.display_mode().unwrap();
            (mode.width(), mode.height())
        }
    }
}

/// Read a window's size from `CGWindowListCopyWindowInfo`, which works for
/// windows owned by any process.
fn window_dimensions_from_cg_list(window_id: u32) -> (u64, u64) {
    use core_foundation::array::CFArray;
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics_helmer_fork::window::{
        copy_window_info, kCGWindowBounds, kCGWindowNumber, kCGWindowListOptionIncludingWindow,
    };

    unsafe {
        let Some(array) = copy_window_info(kCGWindowListOptionIncludingWindow, window_id) else {
            return (0, 0);
        };
        for i in 0..array.len() {
            // The array entries are type-erased dictionaries; work with raw
            // values and construct the wrappers ourselves.
            let Some(entry) = array.get(i) else {
                continue;
            };
            let entry_ptr = *entry as core_foundation::dictionary::CFDictionaryRef;
            if entry_ptr.is_null() {
                continue;
            }
            let dict: CFDictionary = CFDictionary::wrap_under_get_rule(entry_ptr);
            let Some(number_ptr) = dict.find(kCGWindowNumber as *const std::ffi::c_void) else {
                continue;
            };
            let number = CFNumber::wrap_under_get_rule(*number_ptr as core_foundation::number::CFNumberRef);
            if !number.to_i64().is_some_and(|n| n as u32 == window_id) {
                continue;
            }
            let Some(bounds_ptr) = dict.find(kCGWindowBounds as *const std::ffi::c_void) else {
                continue;
            };
            let bounds: CFDictionary = CFDictionary::wrap_under_get_rule(*bounds_ptr as core_foundation::dictionary::CFDictionaryRef);
            let width = dict_f64(&bounds, "Width");
            let height = dict_f64(&bounds, "Height");
            if let (Some(w), Some(h)) = (width, height) {
                return (w as u64, h as u64);
            }
        }
    }
    (0, 0)
}

/// Read a double from an erased CGWindowList bounds dictionary.
fn dict_f64(dict: &core_foundation::dictionary::CFDictionary, key: &str) -> Option<f64> {
    use core_foundation::base::TCFType;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;

    unsafe {
        let key = CFString::new(key).as_concrete_TypeRef() as *const std::ffi::c_void;
        dict.find(key).map(|ptr| CFNumber::wrap_under_get_rule(*ptr as core_foundation::number::CFNumberRef))
            .and_then(|n| n.to_f64())
    }
}
