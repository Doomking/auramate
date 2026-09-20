mod rhythm;

use rhythm::{RhythmCore, Snapshot};
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

struct AppState {
    core: Mutex<RhythmCore>,
}

#[tauri::command]
fn rhythm_snapshot(state: State<'_, AppState>) -> Snapshot {
    let core = state.core.lock().expect("rhythm core lock");
    core.snapshot(Instant::now())
}

#[tauri::command]
fn rhythm_start_focus(state: State<'_, AppState>) -> Snapshot {
    let mut core = state.core.lock().expect("rhythm core lock");
    core.start_focus(Instant::now());
    core.snapshot(Instant::now())
}

#[tauri::command]
fn rhythm_start_break(state: State<'_, AppState>) -> Snapshot {
    let mut core = state.core.lock().expect("rhythm core lock");
    core.start_break(Instant::now());
    core.snapshot(Instant::now())
}

#[tauri::command]
fn rhythm_pause(state: State<'_, AppState>) -> Snapshot {
    let mut core = state.core.lock().expect("rhythm core lock");
    core.pause(Instant::now());
    core.snapshot(Instant::now())
}

#[tauri::command]
fn rhythm_resume(state: State<'_, AppState>) -> Snapshot {
    let mut core = state.core.lock().expect("rhythm core lock");
    core.resume(Instant::now());
    core.snapshot(Instant::now())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
