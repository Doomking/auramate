//! System tray / menu bar — ambient status and Pause / Skip / Snooze.
//!
//! Deliberately does **not** call `set_focus`, show, or unminimize the main
//! window. System notifications are not used (default off; no escalation).
//!
//! Presentation lives here (adapter), reading `Snapshot` only. macOS shows a
//! compact menu-bar `title`; Windows has no title — status is the tooltip.

use crate::rhythm::{Phase, RhythmCore, Snapshot};
use crate::AppState;
use std::time::{Duration, Instant};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub const TRAY_ID: &str = "auramate";
pub const MENU_PAUSE: &str = "tray-pause";
pub const MENU_SKIP: &str = "tray-skip";
pub const MENU_SNOOZE: &str = "tray-snooze";

/// Tooltip (all platforms). Changes clearly when `should_nudge`.
pub fn tray_tooltip(snap: &Snapshot) -> String {
    let clock = format_mmss(snap.remaining_ms, snap.overrun_ms);
    match snap.phase {
        None => "AuraMate · Ready".into(),
        Some(Phase::Focus) if snap.should_nudge => format!("AuraMate · Break due · {clock}"),
        Some(Phase::Focus) if snap.paused => format!("AuraMate · Focus paused · {clock}"),
        Some(Phase::Focus) => format!("AuraMate · Focus · {clock}"),
        Some(Phase::ShortBreak) if snap.should_nudge => {
            format!("AuraMate · Focus due · {clock}")
        }
        Some(Phase::ShortBreak) if snap.paused => {
            format!("AuraMate · Short Break paused · {clock}")
        }
        Some(Phase::ShortBreak) => format!("AuraMate · Short Break · {clock}"),
        Some(Phase::LongBreak) if snap.should_nudge => format!("AuraMate · Focus due · {clock}"),
        Some(Phase::LongBreak) if snap.paused => format!("AuraMate · Long Break paused · {clock}"),
        Some(Phase::LongBreak) => format!("AuraMate · Long Break · {clock}"),
    }
}

/// Compact macOS menu-bar title (Unsupported on Windows). Phase letter + clock;
/// nudge uses `!` so the bar changes without opening the app.
pub fn tray_title(snap: &Snapshot) -> String {
    let clock = format_mmss(snap.remaining_ms, snap.overrun_ms);
    match snap.phase {
        None => "Ready".into(),
        Some(Phase::Focus) if snap.should_nudge => format!("! {clock}"),
        Some(Phase::Focus) if snap.paused => format!("❚❚ F {clock}"),
        Some(Phase::Focus) => format!("F {clock}"),
        Some(Phase::ShortBreak) if snap.should_nudge => format!("! {clock}"),
        Some(Phase::ShortBreak) if snap.paused => format!("❚❚ B {clock}"),
        Some(Phase::ShortBreak) => format!("B {clock}"),
        Some(Phase::LongBreak) if snap.should_nudge => format!("! {clock}"),
        Some(Phase::LongBreak) if snap.paused => format!("❚❚ L {clock}"),
        Some(Phase::LongBreak) => format!("L {clock}"),
    }
}

/// Tray menu label for the pause/resume item.
pub fn tray_pause_label(snap: &Snapshot) -> &'static str {
    if snap.paused {
        "Resume"
    } else {
        "Pause"
    }
}

fn format_mmss(remaining_ms: u64, overrun_ms: u64) -> String {
    let ms = if overrun_ms > 0 { overrun_ms } else { remaining_ms };
    let total_sec = ms / 1000;
    let m = total_sec / 60;
    let s = total_sec % 60;
    if overrun_ms > 0 {
        format!("+{m}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let pause = MenuItem::with_id(app, MENU_PAUSE, "Pause", true, None::<&str>)?;
    let skip = MenuItem::with_id(app, MENU_SKIP, "Skip", true, None::<&str>)?;
    let snooze = MenuItem::with_id(app, MENU_SNOOZE, "Snooze", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&pause, &skip, &snooze])?;

    {
        let state = app.state::<AppState>();
        *state.tray_pause.lock().expect("tray pause lock") = Some(pause);
    }

    let snap = current_snapshot(app.handle());
    let tooltip = tray_tooltip(&snap);
    let title = tray_title(&snap);

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundled window icon");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip(&tooltip)
        .title(&title)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_PAUSE => {
                let snap = mutate_core(app, |core, now| {
                    if core.snapshot(now).paused {
                        core.resume(now);
                    } else {
                        core.pause(now);
                    }
                });
                apply_tray(app, &snap);
            }
            MENU_SKIP => {
                let snap = mutate_core(app, |core, now| core.skip(now));
                apply_tray(app, &snap);
            }
            MENU_SNOOZE => {
                let snap = mutate_core(app, |core, now| core.snooze(now));
                apply_tray(app, &snap);
            }
            _ => {}
        })
        // No left-click handler that shows / focuses the main window.
        .build(app)?;

    spawn_tray_ticker(app.handle().clone());
    Ok(())
}

pub fn apply_tray(app: &AppHandle, snap: &Snapshot) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tray_tooltip(snap)));
        let _ = tray.set_title(Some(tray_title(snap)));
    }
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(guard) = state.tray_pause.lock() {
            if let Some(item) = guard.as_ref() {
                let _ = item.set_text(tray_pause_label(snap));
            }
        }
    }
}

pub fn refresh_tray_from_state(app: &AppHandle) {
    let snap = current_snapshot(app);
    apply_tray(app, &snap);
}

fn current_snapshot(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let core = state.core.lock().expect("rhythm core lock");
    core.snapshot(Instant::now())
}

fn mutate_core<F>(app: &AppHandle, f: F) -> Snapshot
where
    F: FnOnce(&mut RhythmCore, Instant),
{
    let state = app.state::<AppState>();
    let mut core = state.core.lock().expect("rhythm core lock");
    let now = Instant::now();
    f(&mut core, now);
    core.snapshot(Instant::now())
}

fn spawn_tray_ticker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            refresh_tray_from_state(&handle);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhythm::{RhythmCore, FOCUS_DURATION};

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn tray_tooltip_changes_when_nudge_is_due() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let during = core.snapshot(now);
        assert!(tray_tooltip(&during).contains("Focus"));
        assert!(!tray_tooltip(&during).contains("Break due"));
        assert!(tray_title(&during).starts_with("F "));

        let past = now + FOCUS_DURATION + Duration::from_secs(1);
        let nudged = core.snapshot(past);
        assert!(tray_tooltip(&nudged).contains("Break due"));
        assert!(tray_title(&nudged).starts_with("! "));
        assert_ne!(tray_title(&during), tray_title(&nudged));
    }

    #[test]
    fn tray_title_distinguishes_focus_and_break() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        assert!(tray_title(&core.snapshot(now)).starts_with("F "));
        core.start_break(now + Duration::from_secs(1));
        assert!(tray_title(&core.snapshot(now + Duration::from_secs(1))).starts_with("B "));
    }

    #[test]
    fn tray_pause_label_toggles_with_paused() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        assert_eq!(tray_pause_label(&core.snapshot(now)), "Pause");
        core.pause(now + Duration::from_secs(1));
        assert_eq!(
            tray_pause_label(&core.snapshot(now + Duration::from_secs(1))),
            "Resume"
        );
    }
}
