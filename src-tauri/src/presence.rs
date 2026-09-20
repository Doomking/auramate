//! Presence adapter — coarse idle / sleep signals without reading input content.
//! macOS: `CGEventSourceSecondsSinceLastEventType` + `NSWorkspace` sleep/wake.
//! Other platforms: idle stays Active (no false Away).

use crate::AppState;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const POLL: Duration = Duration::from_secs(1);

pub fn spawn_presence_loop(app: AppHandle) {
    #[cfg(target_os = "macos")]
    register_sleep_wake(app.clone());

    std::thread::spawn(move || loop {
        std::thread::sleep(POLL);
        let idle = read_idle();
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            apply_idle(&handle, idle);
        });
    });
}

fn apply_idle(app: &AppHandle, idle: Duration) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let now = Instant::now();
    let wall = std::time::SystemTime::now();
    let mut core = state.core.lock().expect("rhythm core lock");
    core.observe_idle(now, idle);
    let closed = core.drain_intervals();
    drop(core);
    if !closed.is_empty() {
        if let Ok(hist) = state.history.lock() {
            let _ = hist.append_closed(&closed, now, wall);
        }
    }
}

fn apply_sleep(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let now = Instant::now();
    let wall = std::time::SystemTime::now();
    let mut core = state.core.lock().expect("rhythm core lock");
    core.observe_sleep(now);
    let closed = core.drain_intervals();
    drop(core);
    if !closed.is_empty() {
        if let Ok(hist) = state.history.lock() {
            let _ = hist.append_closed(&closed, now, wall);
        }
    }
}

fn apply_wake(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let now = Instant::now();
    let wall = std::time::SystemTime::now();
    let mut core = state.core.lock().expect("rhythm core lock");
    core.observe_wake(now);
    let closed = core.drain_intervals();
    drop(core);
    if !closed.is_empty() {
        if let Ok(hist) = state.history.lock() {
            let _ = hist.append_closed(&closed, now, wall);
        }
    }
}

#[cfg(target_os = "macos")]
fn read_idle() -> Duration {
    // kCGEventSourceStateHIDSystemState = 1; kCGAnyInputEventType = 0xFFFFFFFF
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state_id: u32, event_type: u32) -> f64;
    }
    const HID_SYSTEM_STATE: u32 = 1;
    const ANY_INPUT: u32 = 0xFFFF_FFFF;
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, ANY_INPUT) };
    if secs.is_finite() && secs > 0.0 {
        Duration::from_secs_f64(secs.min(u64::MAX as f64))
    } else {
        Duration::ZERO
    }
}

#[cfg(not(target_os = "macos"))]
fn read_idle() -> Duration {
    Duration::ZERO
}

#[cfg(target_os = "macos")]
fn register_sleep_wake(app: AppHandle) {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{
        NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification,
    };
    use objc2_foundation::{NSNotification, NSNotificationCenter};
    use std::ptr::NonNull;

    let workspace = NSWorkspace::sharedWorkspace();
    let center: Retained<NSNotificationCenter> = workspace.notificationCenter();

    let app_sleep = app.clone();
    let sleep_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
        apply_sleep(&app_sleep);
    });
    let app_wake = app.clone();
    let wake_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
        apply_wake(&app_wake);
    });

    unsafe {
        let sleep_obs = center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceWillSleepNotification),
            None,
            None,
            &sleep_block,
        );
        let wake_obs = center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidWakeNotification),
            None,
            None,
            &wake_block,
        );
        // Keep observer tokens + blocks alive for process lifetime.
        std::mem::forget(sleep_obs);
        std::mem::forget(wake_obs);
        std::mem::forget(sleep_block);
        std::mem::forget(wake_block);
    }
}
