//! macOS workspace lifecycle notifications for Local Link.
//!
//! Network-path monitoring remains behind the `NetworkPathReporter` seam in
//! `lifecycle.rs`. This adapter uses Network.framework plus the existing
//! objc2/AppKit/Foundation stack and a direct `block2` callback dependency.

use std::ffi::{c_char, c_void};
use std::sync::Arc;

use block2::{Block, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass};
use objc2_app_kit::{
    NSApplicationWillTerminateNotification, NSWorkspace, NSWorkspaceDidWakeNotification,
    NSWorkspaceSessionDidBecomeActiveNotification, NSWorkspaceSessionDidResignActiveNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol};

use super::lifecycle::{LifecycleEvent, LifecycleEventSink, NetworkPathReporter, NetworkPathState};

#[link(name = "Network", kind = "framework")]
unsafe extern "C" {
    fn nw_path_monitor_create() -> *mut c_void;
    fn nw_path_monitor_set_update_handler(
        monitor: *mut c_void,
        handler: &Block<dyn Fn(*mut c_void)>,
    );
    fn nw_path_monitor_set_queue(monitor: *mut c_void, queue: *mut c_void);
    fn nw_path_monitor_start(monitor: *mut c_void);
    fn nw_path_monitor_cancel(monitor: *mut c_void);
    fn nw_path_get_status(path: *mut c_void) -> i32;
}

unsafe extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
}

/// Owns a Network.framework path monitor and its private serial queue. Both
/// objects are Objective-C reference-counted OS objects represented as
/// AnyObject so Retained releases them on Drop.
struct MacOsNetworkPathMonitor {
    monitor: Retained<AnyObject>,
    _queue: Retained<AnyObject>,
}

// SAFETY: NWPathMonitor and dispatch queues are documented thread-safe OS
// objects. All mutation/callback delivery is serialized by the private queue;
// Rust never dereferences their AnyObject representation or accesses mutable
// Objective-C ivars.
unsafe impl Send for MacOsNetworkPathMonitor {}
unsafe impl Sync for MacOsNetworkPathMonitor {}

impl MacOsNetworkPathMonitor {
    fn start(sink: LifecycleEventSink) -> Option<Self> {
        // Take ownership immediately after each successful allocation. If the
        // queue allocation fails, the already-retained monitor is dropped; a
        // monitor allocation failure never allocates a queue in the first
        // place, so neither partial-initialization path leaks a raw OS object.
        // SAFETY: the create function returns a retained OS object or null.
        let monitor_pointer = unsafe { nw_path_monitor_create() };
        // SAFETY: Network objects support Objective-C retain/release and this
        // pointer carries +1 ownership.
        let monitor = unsafe { Retained::from_raw(monitor_pointer.cast::<AnyObject>()) }?;
        // SAFETY: the create function returns a retained OS object or null.
        let queue_pointer = unsafe {
            dispatch_queue_create(
                c"com.clipriva.desktop.local-link-network-path".as_ptr(),
                std::ptr::null(),
            )
        };
        // SAFETY: dispatch objects support Objective-C retain/release and this
        // pointer carries +1 ownership. Returning here drops `monitor`.
        let queue = unsafe { Retained::from_raw(queue_pointer.cast::<AnyObject>()) }?;
        let reporter = NetworkPathReporter::new(sink);
        let handler: RcBlock<dyn Fn(*mut c_void)> = RcBlock::new(move |path| {
            // `nw_path_status_satisfied` is 1. Satisfiable and invalid paths
            // are not currently usable and therefore suspend transport until
            // a later satisfied callback.
            let status = unsafe { nw_path_get_status(path) };
            reporter.report(if status == 1 {
                NetworkPathState::Satisfied
            } else {
                NetworkPathState::Unsatisfied
            });
        });
        // SAFETY: monitor, queue and copied handler remain valid for the
        // monitor lifetime; Network.framework serializes callbacks on queue.
        unsafe {
            nw_path_monitor_set_update_handler(
                Retained::as_ptr(&monitor).cast_mut().cast(),
                &handler,
            );
            nw_path_monitor_set_queue(
                Retained::as_ptr(&monitor).cast_mut().cast(),
                Retained::as_ptr(&queue).cast_mut().cast(),
            );
            nw_path_monitor_start(Retained::as_ptr(&monitor).cast_mut().cast());
        }
        Some(Self {
            monitor,
            _queue: queue,
        })
    }
}

impl Drop for MacOsNetworkPathMonitor {
    fn drop(&mut self) {
        // SAFETY: this is the live monitor created in start. Cancellation is
        // idempotent and prevents future path callbacks.
        unsafe {
            nw_path_monitor_cancel(Retained::as_ptr(&self.monitor).cast_mut().cast());
        }
    }
}

struct WorkspaceObserverIvars {
    sink: LifecycleEventSink,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. The only ivar is an
    // Arc-backed Send + Sync callback, and each Objective-C selector forwards
    // a content-free enum value without retaining the notification.
    #[unsafe(super(NSObject))]
    #[name = "ClipRivaLocalLinkWorkspaceObserver"]
    #[ivars = WorkspaceObserverIvars]
    struct WorkspaceObserver;

    impl WorkspaceObserver {
        #[unsafe(method(cliprivaLocalLinkWillSleep:))]
        fn will_sleep(&self, _notification: &NSNotification) {
            (self.ivars().sink)(LifecycleEvent::WillSleep);
        }

        #[unsafe(method(cliprivaLocalLinkDidWake:))]
        fn did_wake(&self, _notification: &NSNotification) {
            (self.ivars().sink)(LifecycleEvent::DidWake);
        }

        #[unsafe(method(cliprivaLocalLinkSessionDidResignActive:))]
        fn session_did_resign_active(&self, _notification: &NSNotification) {
            (self.ivars().sink)(LifecycleEvent::SessionInactive);
        }

        #[unsafe(method(cliprivaLocalLinkSessionDidBecomeActive:))]
        fn session_did_become_active(&self, _notification: &NSNotification) {
            (self.ivars().sink)(LifecycleEvent::SessionActive);
        }

        #[unsafe(method(cliprivaLocalLinkWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            (self.ivars().sink)(LifecycleEvent::WillTerminate);
        }
    }

    unsafe impl NSObjectProtocol for WorkspaceObserver {}
);

impl WorkspaceObserver {
    fn new(sink: LifecycleEventSink) -> Retained<Self> {
        let observer = Self::alloc().set_ivars(WorkspaceObserverIvars { sink });
        // SAFETY: `observer` has initialized ivars and NSObject's `init` has no
        // additional preconditions.
        unsafe { msg_send![super(observer), init] }
    }
}

/// Owns the notification registrations. Dropping the handle unregisters all
/// observers before releasing the callback object.
pub(crate) struct MacOsLifecycleMonitor {
    observer: Retained<WorkspaceObserver>,
    workspace_center: Retained<NSNotificationCenter>,
    application_center: Retained<NSNotificationCenter>,
    _network_path_monitor: Option<MacOsNetworkPathMonitor>,
}

impl MacOsLifecycleMonitor {
    pub(crate) fn start(sink: LifecycleEventSink) -> Self {
        let observer = WorkspaceObserver::new(Arc::clone(&sink));
        let workspace = NSWorkspace::sharedWorkspace();
        let workspace_center = workspace.notificationCenter();
        let application_center = NSNotificationCenter::defaultCenter();

        // SAFETY: Every selector is implemented by WorkspaceObserver with the
        // standard one-NSNotification argument signature. Centers are retained
        // for at least as long as the observer registration and removed in Drop.
        unsafe {
            workspace_center.addObserver_selector_name_object(
                &observer,
                sel!(cliprivaLocalLinkWillSleep:),
                Some(NSWorkspaceWillSleepNotification),
                None,
            );
            workspace_center.addObserver_selector_name_object(
                &observer,
                sel!(cliprivaLocalLinkDidWake:),
                Some(NSWorkspaceDidWakeNotification),
                None,
            );
            workspace_center.addObserver_selector_name_object(
                &observer,
                sel!(cliprivaLocalLinkSessionDidResignActive:),
                Some(NSWorkspaceSessionDidResignActiveNotification),
                None,
            );
            workspace_center.addObserver_selector_name_object(
                &observer,
                sel!(cliprivaLocalLinkSessionDidBecomeActive:),
                Some(NSWorkspaceSessionDidBecomeActiveNotification),
                None,
            );
            application_center.addObserver_selector_name_object(
                &observer,
                sel!(cliprivaLocalLinkWillTerminate:),
                Some(NSApplicationWillTerminateNotification),
                None,
            );
        }

        let network_path_monitor = MacOsNetworkPathMonitor::start(Arc::clone(&sink));
        if network_path_monitor.is_some() {
            (sink)(LifecycleEvent::NetworkMonitorStarted);
        } else {
            (sink)(LifecycleEvent::NetworkMonitorFailed);
        }

        Self {
            observer,
            workspace_center,
            application_center,
            _network_path_monitor: network_path_monitor,
        }
    }
}

impl Drop for MacOsLifecycleMonitor {
    fn drop(&mut self) {
        // SAFETY: The exact retained observer registered in `start` is still
        // alive, and NSNotificationCenter accepts removal from Drop.
        unsafe {
            self.workspace_center.removeObserver(&self.observer);
            self.application_center.removeObserver(&self.observer);
        }
    }
}
