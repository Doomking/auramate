mod rhythm;

use rhythm::{RhythmCore, Snapshot, EXTEND_FIVE, EXTEND_TEN};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

struct AppState {
    core: Mutex<RhythmCore>,
}

fn with_core<F>(state: &State<'_, AppState>, f: F) -> Snapshot
where
    F: FnOnce(&mut RhythmCore, Instant),
{
    let mut core = state.core.lock().expect("rhythm core lock");
    let now = Instant::now();
    f(&mut core, now);
    core.snapshot(Instant::now())
}

#[tauri::command]
fn rhythm_snapshot(state: State<'_, AppState>) -> Snapshot {
    let core = state.core.lock().expect("rhythm core lock");
    core.snapshot(Instant::now())
}

#[tauri::command]
fn rhythm_start_focus(state: State<'_, AppState>) -> Snapshot {
    with_core(&state, |core, now| core.start_focus(now))
}

#[tauri::command]
fn rhythm_start_break(state: State<'_, AppState>) -> Snapshot {
    with_core(&state, |core, now| core.start_break(now))
}

#[tauri::command]
fn rhythm_pause(state: State<'_, AppState>) -> Snapshot {
    with_core(&state, |core, now| core.pause(now))
}

#[tauri::command]
fn rhythm_resume(state: State<'_, AppState>) -> Snapshot {
    with_core(&state, |core, now| core.resume(now))
}

#[tauri::command]
fn rhythm_extend(state: State<'_, AppState>, minutes: u32) -> Snapshot {
    let by = match minutes {
        5 => EXTEND_FIVE,
        10 => EXTEND_TEN,
        other => Duration::from_secs(u64::from(other) * 60),
    };
    with_core(&state, |core, now| core.extend(now, by))
}

#[tauri::command]
fn rhythm_snooze(state: State<'_, AppState>) -> Snapshot {
    with_core(&state, |core, now| core.snooze(now))
}

#[tauri::command]
fn rhythm_skip(state: State<'_, AppState>) -> Snapshot {
    with_core(&state, |core, now| core.skip(now))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            core: Mutex::new(RhythmCore::new()),
        })
        .invoke_handler(tauri::generate_handler![
            rhythm_snapshot,
            rhythm_start_focus,
            rhythm_start_break,
            rhythm_pause,
            rhythm_resume,
            rhythm_extend,
            rhythm_snooze,
            rhythm_skip,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
