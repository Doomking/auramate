mod checkpoint;
mod history;
mod pet;
mod presence;
mod rhythm;
mod tray;

use checkpoint::{gap_since_quit, wall_ms, CheckpointStore, PersistedCheckpoint};
use history::{HistoryRow, HistoryStore};
use rhythm::{RhythmCore, Snapshot, EXTEND_FIVE, EXTEND_TEN};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, RunEvent, State};

pub(crate) struct AppState {
    pub(crate) core: Mutex<RhythmCore>,
    pub(crate) history: Mutex<HistoryStore>,
    pub(crate) checkpoint: Mutex<CheckpointStore>,
    pub(crate) tray_pause: Mutex<Option<MenuItem<tauri::Wry>>>,
}

fn persist_checkpoint(state: &AppState, core: &RhythmCore, now: Instant, wall: SystemTime) {
    let Ok(store) = state.checkpoint.lock() else {
        return;
    };
    let data = core.export_checkpoint(now).map(|checkpoint| PersistedCheckpoint {
        quit_wall_ms: wall_ms(wall),
        checkpoint,
    });
    let _ = store.save(data.as_ref());
}

fn with_core<F>(state: &State<'_, AppState>, f: F) -> Snapshot
where
    F: FnOnce(&mut RhythmCore, Instant),
{
    let now = Instant::now();
    let wall = SystemTime::now();
    let mut core = state.core.lock().expect("rhythm core lock");
    f(&mut core, now);
    core.tick(Instant::now());
    let closed = core.drain_intervals();
    let snap = core.snapshot(Instant::now());
    persist_checkpoint(state, &core, Instant::now(), SystemTime::now());
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
        let mut core = state.core.lock().expect("rhythm core lock");
        let now = Instant::now();
        core.tick(now);
        let snap = core.snapshot(now);
        persist_checkpoint(&state, &core, now, SystemTime::now());
        snap
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
    with_core_and_tray(&app, &state, |core, now| core.dismiss_recovery_hint(now))
}

#[tauri::command]
fn rhythm_observe_media_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    active: bool,
) -> Snapshot {
    with_core_and_tray(&app, &state, |core, now| {
        core.observe_media_meeting(now, active)
    })
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

fn checkpoint_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir")
        .join("session.json")
}

fn restore_from_checkpoint(state: &AppState) {
    let loaded = {
        let store = state.checkpoint.lock().expect("checkpoint lock");
        store.load()
    };
    let Some(persisted) = loaded else {
        return;
    };
    let now = Instant::now();
    let wall = SystemTime::now();
    let gap = gap_since_quit(persisted.quit_wall_ms, wall);
    let mut core = state.core.lock().expect("rhythm core lock");
    core.relaunch_from_checkpoint(persisted.checkpoint, gap, now);
    let closed = core.drain_intervals();
    persist_checkpoint(state, &core, now, wall);
    drop(core);
    if !closed.is_empty() {
        let hist = state.history.lock().expect("history lock");
        let _ = hist.append_closed(&closed, now, wall);
    }
}

fn persist_on_exit(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let now = Instant::now();
    let wall = SystemTime::now();
    let core = state.core.lock().expect("rhythm core lock");
    persist_checkpoint(&state, &core, now, wall);
}

/// Presence adapter also mutates core — keep the quit file warm.
pub(crate) fn persist_after_presence(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let now = Instant::now();
    let wall = SystemTime::now();
    let core = state.core.lock().expect("rhythm core lock");
    persist_checkpoint(&state, &core, now, wall);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let hist_path = history_path(app.handle());
            let store = HistoryStore::open(&hist_path).expect("open history sqlite");
            let cp_store = CheckpointStore::open(checkpoint_path(app.handle()));
            app.manage(AppState {
                core: Mutex::new(RhythmCore::new()),
                history: Mutex::new(store),
                checkpoint: Mutex::new(cp_store),
                tray_pause: Mutex::new(None),
            });
            restore_from_checkpoint(&*app.state::<AppState>());
            tray::setup_tray(app)?;
            pet::setup_pet_window(app)?;
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
            rhythm_observe_media_meeting,
            history_list,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            persist_on_exit(&app_handle);
        }
    });
}
