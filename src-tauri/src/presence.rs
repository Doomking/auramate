//! Presence adapter — coarse idle / sleep / media signals without reading content.
//! macOS: idle via `CGEventSourceSecondsSinceLastEventType`; sleep via `NSWorkspace`;
//! media/meeting proxy via CoreAudio default-input `DeviceIsRunningSomewhere` (bool only).
//! Camera / DND proxies can be added later; missing signals degrade to “no media/meeting”
//! with no permission prompts.

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
        let media = read_media_meeting();
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            apply_idle(&handle, idle);
            apply_media(&handle, media);
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

fn apply_media(app: &AppHandle, active: bool) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let now = Instant::now();
    let wall = std::time::SystemTime::now();
    let mut core = state.core.lock().expect("rhythm core lock");
    core.observe_media_meeting(now, active);
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

/// Optional media/meeting proxy (macOS: default input running somewhere).
/// Failure or unsupported OS → false (graceful downgrade, no prompts).
fn read_media_meeting() -> bool {
    #[cfg(target_os = "macos")]
    {
        default_input_running_somewhere().unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
fn default_input_running_somewhere() -> Result<bool, ()> {
    // Minimal CoreAudio FFI: default input device + DeviceIsRunningSomewhere.
    // Returns a boolean only — never PID / app name / samples.
    #[repr(C)]
    struct AudioObjectPropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyDataSize(
            object_id: u32,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const std::ffi::c_void,
            out_size: *mut u32,
        ) -> i32;
        fn AudioObjectGetPropertyData(
            object_id: u32,
            address: *const AudioObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const std::ffi::c_void,
            io_data_size: *mut u32,
            out_data: *mut std::ffi::c_void,
        ) -> i32;
    }

    const SYSTEM: u32 = 1; // kAudioObjectSystemObject
    const GLOBAL: u32 = 0x676c6f62; // kAudioObjectPropertyScopeGlobal 'glob'
    const MAIN: u32 = 0; // kAudioObjectPropertyElementMain
    const DEFAULT_INPUT: u32 = 0x64496e20; // kAudioHardwarePropertyDefaultInputDevice 'dIn '
    const RUNNING_SOMEWHERE: u32 = 0x72756e53; // kAudioDevicePropertyDeviceIsRunningSomewhere 'runS'

    let addr_default = AudioObjectPropertyAddress {
        selector: DEFAULT_INPUT,
        scope: GLOBAL,
        element: MAIN,
    };
    let mut device: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM,
            &addr_default,
            0,
            std::ptr::null(),
            &mut size,
            &mut device as *mut u32 as *mut _,
        )
    };
    if status != 0 || device == 0 {
        return Err(());
    }

    let addr_run = AudioObjectPropertyAddress {
        selector: RUNNING_SOMEWHERE,
        scope: GLOBAL,
        element: MAIN,
    };
    let mut running: u32 = 0;
    let mut run_size = std::mem::size_of::<u32>() as u32;
    let mut probe = 0u32;
    let probe_status = unsafe {
        AudioObjectGetPropertyDataSize(device, &addr_run, 0, std::ptr::null(), &mut probe)
    };
    if probe_status != 0 {
        return Err(());
    }
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &addr_run,
            0,
            std::ptr::null(),
            &mut run_size,
            &mut running as *mut u32 as *mut _,
        )
    };
    if status != 0 {
        return Err(());
    }
    Ok(running != 0)
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
        std::mem::forget(sleep_obs);
        std::mem::forget(wake_obs);
        std::mem::forget(sleep_block);
        std::mem::forget(wake_block);
    }
}
