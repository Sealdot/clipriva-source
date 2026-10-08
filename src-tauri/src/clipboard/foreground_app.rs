/// Supplies the name of the application currently owning the foreground.
///
/// Attribution is intentionally best-effort: clipboard capture must remain
/// independent from whether a platform can provide this metadata.
pub trait ForegroundApplicationProvider {
    fn frontmost_application(&self) -> Option<String>;
}

#[derive(Default)]
pub struct PlatformForegroundApplicationProvider;

impl ForegroundApplicationProvider for PlatformForegroundApplicationProvider {
    fn frontmost_application(&self) -> Option<String> {
        platform::frontmost_application()
    }
}

pub(crate) fn frontmost_process_id() -> Option<i32> {
    platform::frontmost_process_id()
}

pub(crate) fn accessibility_is_trusted() -> bool {
    platform::accessibility_is_trusted()
}

pub(crate) fn request_accessibility_permission() -> bool {
    platform::request_accessibility_permission()
}

pub(crate) fn activate_application(process_id: i32) -> bool {
    platform::activate_application(process_id)
}

pub(crate) fn send_command_v() -> bool {
    platform::send_command_v()
}

pub(crate) fn open_accessibility_settings() -> bool {
    platform::open_accessibility_settings()
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{c_char, c_void, CStr};

    type Object = *mut c_void;
    type Selector = *const c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Object;
        fn sel_registerName(name: *const c_char) -> Selector;
        fn objc_msgSend();
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: Object) -> bool;
        fn CGEventCreateKeyboardEvent(source: Object, virtual_key: u16, key_down: bool) -> Object;
        fn CGEventSetFlags(event: Object, flags: u64);
        fn CGEventPost(tap: u32, event: Object);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: Object);
    }

    pub(super) fn frontmost_application() -> Option<String> {
        // NSWorkspace gives us the active app without asking for Accessibility
        // or Automation privileges. All Objective-C calls below can return nil;
        // each is converted to `None` so attribution can never interrupt capture.
        unsafe {
            let autorelease_pool = new_autorelease_pool();
            let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
            let shared_workspace = send_object(workspace_class, selector(b"sharedWorkspace\0"));
            let frontmost_app = send_object(shared_workspace, selector(b"frontmostApplication\0"));
            let localized_name = send_object(frontmost_app, selector(b"localizedName\0"));
            let name = send_c_string(localized_name, selector(b"UTF8String\0"));

            let application_name = if name.is_null() {
                None
            } else {
                CStr::from_ptr(name)
                    .to_str()
                    .ok()
                    .and_then(normalize_application_name)
            };

            send_void(autorelease_pool, selector(b"drain\0"));
            application_name
        }
    }

    pub(super) fn frontmost_process_id() -> Option<i32> {
        unsafe {
            let autorelease_pool = new_autorelease_pool();
            let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
            let shared_workspace = send_object(workspace_class, selector(b"sharedWorkspace\0"));
            let frontmost_app = send_object(shared_workspace, selector(b"frontmostApplication\0"));
            let process_id = send_i32(frontmost_app, selector(b"processIdentifier\0"));
            send_void(autorelease_pool, selector(b"drain\0"));
            (process_id > 0).then_some(process_id)
        }
    }

    pub(super) fn accessibility_is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub(super) fn request_accessibility_permission() -> bool {
        unsafe {
            let autorelease_pool = new_autorelease_pool();
            let string_class = objc_getClass(c"NSString".as_ptr());
            let prompt_key = send_object_c_string(
                string_class,
                selector(b"stringWithUTF8String:\0"),
                c"AXTrustedCheckOptionPrompt".as_ptr(),
            );
            let number_class = objc_getClass(c"NSNumber".as_ptr());
            let prompt_value = send_object_bool(number_class, selector(b"numberWithBool:\0"), true);
            let dictionary_class = objc_getClass(c"NSDictionary".as_ptr());
            let options = send_object_two_objects(
                dictionary_class,
                selector(b"dictionaryWithObject:forKey:\0"),
                prompt_value,
                prompt_key,
            );
            let trusted = AXIsProcessTrustedWithOptions(options);
            send_void(autorelease_pool, selector(b"drain\0"));
            trusted
        }
    }

    pub(super) fn activate_application(process_id: i32) -> bool {
        if process_id <= 0 {
            return false;
        }

        unsafe {
            let autorelease_pool = new_autorelease_pool();
            let application_class = objc_getClass(c"NSRunningApplication".as_ptr());
            let application = send_object_i32(
                application_class,
                selector(b"runningApplicationWithProcessIdentifier:\0"),
                process_id,
            );
            let activated = send_bool_usize(application, selector(b"activateWithOptions:\0"), 0);
            send_void(autorelease_pool, selector(b"drain\0"));
            activated
        }
    }

    pub(super) fn send_command_v() -> bool {
        const CG_HID_EVENT_TAP: u32 = 0;
        const COMMAND_FLAG: u64 = 0x0010_0000;
        const KEYCODE_V: u16 = 9;

        unsafe {
            let key_down = CGEventCreateKeyboardEvent(std::ptr::null_mut(), KEYCODE_V, true);
            let key_up = CGEventCreateKeyboardEvent(std::ptr::null_mut(), KEYCODE_V, false);
            if key_down.is_null() || key_up.is_null() {
                if !key_down.is_null() {
                    CFRelease(key_down);
                }
                if !key_up.is_null() {
                    CFRelease(key_up);
                }
                return false;
            }

            CGEventSetFlags(key_down, COMMAND_FLAG);
            CGEventSetFlags(key_up, COMMAND_FLAG);
            CGEventPost(CG_HID_EVENT_TAP, key_down);
            CGEventPost(CG_HID_EVENT_TAP, key_up);
            CFRelease(key_down);
            CFRelease(key_up);
            true
        }
    }

    pub(super) fn open_accessibility_settings() -> bool {
        unsafe {
            let autorelease_pool = new_autorelease_pool();
            let string_class = objc_getClass(c"NSString".as_ptr());
            let settings_url = send_object_c_string(
                string_class,
                selector(b"stringWithUTF8String:\0"),
                c"x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
                    .as_ptr(),
            );
            let url_class = objc_getClass(c"NSURL".as_ptr());
            let url = send_object_object(url_class, selector(b"URLWithString:\0"), settings_url);
            let workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
            let workspace = send_object(workspace_class, selector(b"sharedWorkspace\0"));
            let opened = send_bool_object(workspace, selector(b"openURL:\0"), url);
            send_void(autorelease_pool, selector(b"drain\0"));
            opened
        }
    }

    unsafe fn new_autorelease_pool() -> Object {
        let pool_class = unsafe { objc_getClass(c"NSAutoreleasePool".as_ptr()) };
        let allocated_pool = unsafe { send_object(pool_class, selector(b"alloc\0")) };
        unsafe { send_object(allocated_pool, selector(b"init\0")) }
    }

    unsafe fn selector(name: &'static [u8]) -> Selector {
        unsafe { sel_registerName(name.as_ptr().cast()) }
    }

    unsafe fn send_object(receiver: Object, selector: Selector) -> Object {
        if receiver.is_null() || selector.is_null() {
            return std::ptr::null_mut();
        }

        let message: unsafe extern "C" fn(Object, Selector) -> Object =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector) }
    }

    unsafe fn send_c_string(receiver: Object, selector: Selector) -> *const c_char {
        if receiver.is_null() || selector.is_null() {
            return std::ptr::null();
        }

        let message: unsafe extern "C" fn(Object, Selector) -> *const c_char =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector) }
    }

    unsafe fn send_i32(receiver: Object, selector: Selector) -> i32 {
        if receiver.is_null() || selector.is_null() {
            return 0;
        }

        let message: unsafe extern "C" fn(Object, Selector) -> i32 =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector) }
    }

    unsafe fn send_object_i32(receiver: Object, selector: Selector, value: i32) -> Object {
        if receiver.is_null() || selector.is_null() {
            return std::ptr::null_mut();
        }

        let message: unsafe extern "C" fn(Object, Selector, i32) -> Object =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector, value) }
    }

    unsafe fn send_object_object(receiver: Object, selector: Selector, value: Object) -> Object {
        if receiver.is_null() || selector.is_null() || value.is_null() {
            return std::ptr::null_mut();
        }

        let message: unsafe extern "C" fn(Object, Selector, Object) -> Object =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector, value) }
    }

    unsafe fn send_object_bool(receiver: Object, selector: Selector, value: bool) -> Object {
        if receiver.is_null() || selector.is_null() {
            return std::ptr::null_mut();
        }

        let message: unsafe extern "C" fn(Object, Selector, bool) -> Object =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector, value) }
    }

    unsafe fn send_object_c_string(
        receiver: Object,
        selector: Selector,
        value: *const c_char,
    ) -> Object {
        if receiver.is_null() || selector.is_null() || value.is_null() {
            return std::ptr::null_mut();
        }

        let message: unsafe extern "C" fn(Object, Selector, *const c_char) -> Object =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector, value) }
    }

    unsafe fn send_object_two_objects(
        receiver: Object,
        selector: Selector,
        first: Object,
        second: Object,
    ) -> Object {
        if receiver.is_null() || selector.is_null() || first.is_null() || second.is_null() {
            return std::ptr::null_mut();
        }

        let message: unsafe extern "C" fn(Object, Selector, Object, Object) -> Object =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector, first, second) }
    }

    unsafe fn send_bool_usize(receiver: Object, selector: Selector, value: usize) -> bool {
        if receiver.is_null() || selector.is_null() {
            return false;
        }

        let message: unsafe extern "C" fn(Object, Selector, usize) -> bool =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector, value) }
    }

    unsafe fn send_bool_object(receiver: Object, selector: Selector, value: Object) -> bool {
        if receiver.is_null() || selector.is_null() || value.is_null() {
            return false;
        }

        let message: unsafe extern "C" fn(Object, Selector, Object) -> bool =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector, value) }
    }

    unsafe fn send_void(receiver: Object, selector: Selector) {
        if receiver.is_null() || selector.is_null() {
            return;
        }

        let message: unsafe extern "C" fn(Object, Selector) =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { message(receiver, selector) }
    }

    fn normalize_application_name(name: &str) -> Option<String> {
        let name = name.trim();
        (!name.is_empty()).then(|| name.to_owned())
    }

    #[cfg(test)]
    mod tests {
        use super::normalize_application_name;

        #[test]
        fn normalizes_a_frontmost_application_name() {
            assert_eq!(
                normalize_application_name("  Safari\n"),
                Some("Safari".to_owned())
            );
        }

        #[test]
        fn ignores_an_empty_frontmost_application_name() {
            assert_eq!(normalize_application_name(" \t\n"), None);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub(super) fn frontmost_application() -> Option<String> {
        None
    }

    pub(super) fn frontmost_process_id() -> Option<i32> {
        None
    }

    pub(super) fn accessibility_is_trusted() -> bool {
        false
    }

    pub(super) fn request_accessibility_permission() -> bool {
        false
    }

    pub(super) fn activate_application(_process_id: i32) -> bool {
        false
    }

    pub(super) fn send_command_v() -> bool {
        false
    }

    pub(super) fn open_accessibility_settings() -> bool {
        false
    }

    #[cfg(test)]
    mod tests {
        use super::frontmost_application;

        #[test]
        fn reports_no_application_when_the_platform_has_no_provider() {
            assert_eq!(frontmost_application(), None);
        }
    }
}
