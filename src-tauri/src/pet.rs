//! Pet ambient window — non-activating, transparent, monitor-edge companion.
//! Renders core `PetState` only; no second corner HUD.

use crate::rhythm::PetState;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    webview::WebviewWindowBuilder, AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
};

pub const PET_LABEL: &str = "pet";
pub const PET_WIDTH: u32 = 120;
pub const PET_HEIGHT: u32 = 120;
const MARGIN: i32 = 16;
const FOLLOW_POLL: Duration = Duration::from_millis(500);

/// Last monitor identity the pet was placed on (name or geometry key).
pub(crate) struct PetFollowState {
    pub last_monitor_key: Option<String>,
}

/// Bottom-right of the work area, with margin (physical pixels).
pub fn pet_edge_position(
    work_x: i32,
    work_y: i32,
    work_w: u32,
    work_h: u32,
    pet_w: u32,
    pet_h: u32,
    margin: i32,
) -> (i32, i32) {
    let x = work_x + work_w as i32 - pet_w as i32 - margin;
    let y = work_y + work_h as i32 - pet_h as i32 - margin;
    (x, y)
}

/// Move only when the monitor under the pointer changes.
pub fn should_reposition(prev_key: Option<&str>, current_key: &str) -> bool {
    prev_key != Some(current_key)
}

pub fn setup_pet_window(app: &tauri::App) -> tauri::Result<()> {
    app.manage(Mutex::new(PetFollowState {
        last_monitor_key: None,
    }));

    let window = WebviewWindowBuilder::new(app, PET_LABEL, WebviewUrl::App("index.html#pet".into()))
        .title("AuraMate Pet")
        .inner_size(PET_WIDTH as f64, PET_HEIGHT as f64)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(true)
        .build()?;

    // Never steal focus after creation updates.
    let _ = window.set_focusable(false);

    place_on_pointer_monitor(app.handle())?;
    spawn_follow_loop(app.handle().clone());
    Ok(())
}

fn spawn_follow_loop(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(FOLLOW_POLL);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = place_on_pointer_monitor(&handle);
        });
    });
}

fn place_on_pointer_monitor(app: &AppHandle) -> tauri::Result<()> {
    let cursor = app.cursor_position()?;
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)?
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let key = monitor
        .name()
        .cloned()
        .unwrap_or_else(|| {
            format!(
                "{}:{}:{}x{}",
                monitor.position().x,
                monitor.position().y,
                monitor.size().width,
                monitor.size().height
            )
        });

    let state = app.state::<Mutex<PetFollowState>>();
    let mut follow = state.lock().expect("pet follow lock");
    if !should_reposition(follow.last_monitor_key.as_deref(), &key) {
        return Ok(());
    }
    follow.last_monitor_key = Some(key);
    drop(follow);

    let work = monitor.work_area();
    let (x, y) = pet_edge_position(
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
        PET_WIDTH,
        PET_HEIGHT,
        MARGIN,
    );

    if let Some(pet) = app.get_webview_window(PET_LABEL) {
        let _ = pet.set_position(tauri::Position::Physical(PhysicalPosition { x, y }));
        let _ = pet.set_size(tauri::Size::Physical(PhysicalSize {
            width: PET_WIDTH,
            height: PET_HEIGHT,
        }));
        let _ = pet.set_focusable(false);
    }
    Ok(())
}

/// CSS class / label helper for the pet surface (adapter-side, tested).
#[cfg(test)]
pub fn pet_surface_label(state: PetState) -> &'static str {
    match state {
        PetState::Idle => "idle",
        PetState::Nudge => "nudge",
        PetState::Rest => "rest",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_position_is_bottom_right_with_margin() {
        let (x, y) = pet_edge_position(100, 50, 1920, 1080, 120, 120, 16);
        assert_eq!(x, 100 + 1920 - 120 - 16);
        assert_eq!(y, 50 + 1080 - 120 - 16);
    }

    #[test]
    fn only_moves_when_monitor_key_changes() {
        assert!(should_reposition(None, "Built-in"));
        assert!(!should_reposition(Some("Built-in"), "Built-in"));
        assert!(should_reposition(Some("Built-in"), "External"));
    }

    #[test]
    fn surface_labels_match_states() {
        assert_eq!(pet_surface_label(PetState::Idle), "idle");
        assert_eq!(pet_surface_label(PetState::Nudge), "nudge");
        assert_eq!(pet_surface_label(PetState::Rest), "rest");
    }
}
