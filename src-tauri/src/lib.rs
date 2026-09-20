mod history;
mod presence;
mod rhythm;
mod tray;

use history::{HistoryRow, HistoryStore};
use rhythm::{RhythmCore, Snapshot, EXTEND_FIVE, EXTEND_TEN};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, State};

pub(crate) struct AppState {
    pub(crate) core: Mutex<RhythmCore>,
    pub(crate) history: Mutex<HistoryStore>,
    pub(crate) tray_pause: Mutex<Option<MenuItem<tauri::Wry>>>,
}

fn with_core<F>(state: &State<'_, AppState>, f: F) -> Snapshot
where
    F: FnOnce(&mut RhythmCore, Instant),
{
    let now = Instant::now();
    let wall = SystemTime::now();
    let mut core = state.core.lock().expect("rhythm core lock");
    f(&mut core, now);
    let closed = core.drain_intervals();
    let snap = core.snapshot(Instant::now());
    drop(core);
    if !closed.is_empty() {
        let hist = state.history.lock().expect("history lock");
        let _ = hist.append_closed(&closed, now, wall);
    }
    snap
}

fn with_core_and_tray<F>(app: &AppHandle, state: &State<'_, AppState>, f: F) -> Snapshot
where
    F: FnOnce(&mut RhythmCore, Instant),
{
    let snap = with_core(state, f);
    tray::apply_tray(app, &snap);
    snap
}

#[tauri::command]
fn rhythm_snapshot(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    let snap = {
        let core = state.core.lock().expect("rhythm core lock");
        core.snapshot(Instant::now())
    };
    tray::apply_tray(&app, &snap);
    snap
}

#[tauri::command]
fn rhythm_start_focus(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    with_core_and_tray(&app, &state, |core, now| core.start_focus(now))
}

#[tauri::command]
fn rhythm_start_break(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    with_core_and_tray(&app, &state, |core, now| core.start_break(now))
}

#[tauri::command]
fn rhythm_pause(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    with_core_and_tray(&app, &state, |core, now| core.pause(now))
}

#[tauri::command]
fn rhythm_resume(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    with_core_and_tray(&app, &state, |core, now| core.resume(now))
}

#[tauri::command]
fn rhythm_extend(app: AppHandle, state: State<'_, AppState>, minutes: u32) -> Snapshot {
    let by = match minutes {
        5 => EXTEND_FIVE,
        10 => EXTEND_TEN,
        other => Duration::from_secs(u64::from(other) * 60),
    };
    with_core_and_tray(&app, &state, |core, now| core.extend(now, by))
}

#[tauri::command]
fn rhythm_snooze(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    with_core_and_tray(&app, &state, |core, now| core.snooze(now))
}

#[tauri::command]
fn rhythm_skip(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    with_core_and_tray(&app, &state, |core, now| core.skip(now))
}

#[tauri::command]
fn rhythm_dismiss_recovery(app: AppHandle, state: State<'_, AppState>) -> Snapshot {
    with_core_and_tray(&app, &state, |core, _now| core.dismiss_recovery_hint())
}

#[tauri::command]
fn history_list(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<HistoryRow>, String> {
    let hist = state.history.lock().expect("history lock");
    hist.list_recent(limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

fn history_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir")
        .join("history.sqlite")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let path = history_path(app.handle());
            let store = HistoryStore::open(&path).expect("open history sqlite");
            app.manage(AppState {
                core: Mutex::new(RhythmCore::new()),
                history: Mutex::new(store),
                tray_pause: Mutex::new(None),
            });
            tray::setup_tray(app)?;
            presence::spawn_presence_loop(app.handle().clone());
            Ok(())
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
            rhythm_dismiss_recovery,
            history_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
