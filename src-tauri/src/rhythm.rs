//! Rhythm Core: Focus / Break, presence, 恢复提示, and history interval intents.
//! Public seam for phase-1 behaviour (see `docs/spec.md`).

use serde::Serialize;
use std::time::{Duration, Instant};

/// Default Focus length (spec).
pub const FOCUS_DURATION: Duration = Duration::from_secs(25 * 60);
/// Default Short Break length (spec).
pub const SHORT_BREAK_DURATION: Duration = Duration::from_secs(5 * 60);
/// Default Long Break length (spec).
pub const LONG_BREAK_DURATION: Duration = Duration::from_secs(15 * 60);
/// Focus 时段s before a Long Break.
pub const FOCUSES_PER_LONG_BREAK: u32 = 4;
/// 加时 +5.
pub const EXTEND_FIVE: Duration = Duration::from_secs(5 * 60);
/// 加时 +10.
pub const EXTEND_TEN: Duration = Duration::from_secs(10 * 60);
/// 延后：推迟下一次提醒，不延长时段。
pub const SNOOZE_DELAY: Duration = Duration::from_secs(5 * 60);
/// ~30s idle: silence 轻触, Focus continues.
pub const IDLE_QUIET: Duration = Duration::from_secs(30);
/// ~5 min idle: 离开 = Start Break.
pub const IDLE_AWAY: Duration = Duration::from_secs(5 * 60);

/// Fixed 恢复提示 pool — core picks 1–3 per intentional break.
pub const RECOVERY_POOL: &[&str] = &[
    "Drink some water",
    "Stand and stretch",
    "Look into the distance",
    "Take a short walk",
    "Rest your eyes",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Active,
    Away,
    LockedSleeping,
    Returned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntervalKind {
    Focus,
    ShortBreak,
    LongBreak,
    Overrun,
    Away,
    LockedSleeping,
}

/// Closed interval intent for the History adapter (wall-clock stamped outside core).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedInterval {
    pub kind: IntervalKind,
    pub start: Instant,
    pub end: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub phase: Option<Phase>,
    pub remaining_ms: u64,
    pub overrun_ms: u64,
    pub paused: bool,
    pub focuses_completed_in_cycle: u32,
    pub should_nudge: bool,
    pub presence: Presence,
    /// One-shot 恢复提示 after intentional Start Break while present.
    pub show_recovery_hint: bool,
    pub recovery_suggestions: Vec<String>,
}

#[derive(Debug, Clone)]
struct Session {
    phase: Phase,
    planned_end: Instant,
    paused_remaining: Option<Duration>,
    snooze_until: Option<Instant>,
}

#[derive(Debug, Clone)]
struct OpenInterval {
    kind: IntervalKind,
    start: Instant,
}

/// Deep module owning 阶段 / 存在 / 恢复提示 / history intents.
#[derive(Debug, Clone)]
pub struct RhythmCore {
    session: Option<Session>,
    focuses_completed_in_cycle: u32,
    presence: Presence,
    /// Last observed idle; ~30s silences 轻触 while still Active.
    last_idle: Duration,
    show_recovery_hint: bool,
    recovery_suggestions: Vec<String>,
    suggestion_nonce: u32,
    /// After 归来, allow at most one Start-Focus 轻触 if Break already ended.
    return_nudge_armed: bool,
    open_session_interval: Option<OpenInterval>,
    open_overrun: Option<OpenInterval>,
    open_presence_interval: Option<OpenInterval>,
    closed_intervals: Vec<ClosedInterval>,
}

impl Default for RhythmCore {
    fn default() -> Self {
        Self::new()
    }
}

impl RhythmCore {
    pub fn new() -> Self {
        Self {
            session: None,
            focuses_completed_in_cycle: 0,
            presence: Presence::Active,
            last_idle: Duration::ZERO,
            show_recovery_hint: false,
            recovery_suggestions: Vec::new(),
            suggestion_nonce: 0,
            return_nudge_armed: false,
            open_session_interval: None,
            open_overrun: None,
            open_presence_interval: None,
            closed_intervals: Vec::new(),
        }
    }

    pub fn snapshot(&self, now: Instant) -> Snapshot {
        let Some(session) = &self.session else {
            return Snapshot {
                phase: None,
                remaining_ms: 0,
                overrun_ms: 0,
                paused: false,
                focuses_completed_in_cycle: self.focuses_completed_in_cycle,
                should_nudge: self.idle_nudge(false, false),
                presence: self.presence,
                show_recovery_hint: self.show_recovery_hint,
                recovery_suggestions: self.recovery_suggestions.clone(),
            };
        };

        let paused = session.paused_remaining.is_some();
        let (remaining, overrun) = if let Some(rem) = session.paused_remaining {
            (rem, Duration::ZERO)
        } else if now < session.planned_end {
            (session.planned_end - now, Duration::ZERO)
        } else {
            (Duration::ZERO, now - session.planned_end)
        };

        let past_end = !paused && now >= session.planned_end;
        let snooze_active = session
            .snooze_until
            .map(|until| now < until)
            .unwrap_or(false);

        Snapshot {
            phase: Some(session.phase),
            remaining_ms: remaining.as_millis() as u64,
            overrun_ms: overrun.as_millis() as u64,
            paused,
            focuses_completed_in_cycle: self.focuses_completed_in_cycle,
            should_nudge: self.idle_nudge(past_end && !snooze_active, past_end),
            presence: self.presence,
            show_recovery_hint: self.show_recovery_hint,
            recovery_suggestions: self.recovery_suggestions.clone(),
        }
    }

    fn idle_nudge(&self, boundary_nudge: bool, break_past_end: bool) -> bool {
        // Brief idle (~30s): still Active, but silence 轻触.
        if self.last_idle >= IDLE_QUIET
            && !matches!(
                self.presence,
                Presence::Away | Presence::LockedSleeping | Presence::Returned
            )
        {
            return false;
        }
        match self.presence {
            Presence::Away | Presence::LockedSleeping => false,
            Presence::Returned => self.return_nudge_armed && break_past_end,
            Presence::Active => boundary_nudge,
        }
    }

    /// Drain closed history intervals since last drain (adapter stamps wall clock).
    pub fn drain_intervals(&mut self) -> Vec<ClosedInterval> {
        std::mem::take(&mut self.closed_intervals)
    }

    fn close_open(slot: &mut Option<OpenInterval>, now: Instant, out: &mut Vec<ClosedInterval>) {
        if let Some(open) = slot.take() {
            out.push(ClosedInterval {
                kind: open.kind,
                start: open.start,
                end: now,
            });
        }
    }

    fn open_session(&mut self, kind: IntervalKind, now: Instant) {
        Self::close_open(
            &mut self.open_session_interval,
            now,
            &mut self.closed_intervals,
        );
        Self::close_open(&mut self.open_overrun, now, &mut self.closed_intervals);
        self.open_session_interval = Some(OpenInterval { kind, start: now });
    }

    fn sync_overrun(&mut self, now: Instant) {
        let Some(session) = &self.session else {
            Self::close_open(&mut self.open_overrun, now, &mut self.closed_intervals);
            return;
        };
        let paused = session.paused_remaining.is_some();
        let past_end = !paused && now >= session.planned_end;
        if past_end {
            if self.open_overrun.is_none() {
                self.open_overrun = Some(OpenInterval {
                    kind: IntervalKind::Overrun,
                    start: session.planned_end,
                });
            }
        } else {
            Self::close_open(&mut self.open_overrun, now, &mut self.closed_intervals);
        }
    }

    fn open_presence_kind(&mut self, kind: IntervalKind, now: Instant) {
        Self::close_open(
            &mut self.open_presence_interval,
            now,
            &mut self.closed_intervals,
        );
        self.open_presence_interval = Some(OpenInterval { kind, start: now });
    }

    fn clear_presence_interval(&mut self, now: Instant) {
        Self::close_open(
            &mut self.open_presence_interval,
            now,
            &mut self.closed_intervals,
        );
    }

    fn pick_recovery(&mut self) -> Vec<String> {
        let count = 1 + (self.suggestion_nonce % 3) as usize;
        self.suggestion_nonce = self.suggestion_nonce.wrapping_add(1);
        let start = (self.suggestion_nonce as usize) % RECOVERY_POOL.len();
        (0..count)
            .map(|i| RECOVERY_POOL[(start + i) % RECOVERY_POOL.len()].to_string())
            .collect()
    }

    /// Start a Focus 时段 from Idle or after a Break.
    pub fn start_focus(&mut self, now: Instant) {
        self.show_recovery_hint = false;
        self.recovery_suggestions.clear();
        self.return_nudge_armed = false;
        if self.presence == Presence::Returned {
            self.presence = Presence::Active;
        }
        self.open_session(IntervalKind::Focus, now);
        self.session = Some(Session {
            phase: Phase::Focus,
            planned_end: now + FOCUS_DURATION,
            paused_remaining: None,
            snooze_until: None,
        });
        self.sync_overrun(now);
    }

    /// End Focus and start Short or Long Break (after every fourth Focus).
    pub fn start_break(&mut self, now: Instant) {
        self.start_break_inner(now, false);
    }

    fn start_break_inner(&mut self, now: Instant, from_away: bool) {
        let Some(session) = &self.session else {
            return;
        };
        if session.phase != Phase::Focus {
            return;
        }

        self.focuses_completed_in_cycle += 1;
        let (phase, duration, kind) = if self.focuses_completed_in_cycle >= FOCUSES_PER_LONG_BREAK
        {
            self.focuses_completed_in_cycle = 0;
            (
                Phase::LongBreak,
                LONG_BREAK_DURATION,
                IntervalKind::LongBreak,
            )
        } else {
            (
                Phase::ShortBreak,
                SHORT_BREAK_DURATION,
                IntervalKind::ShortBreak,
            )
        };

        self.open_session(kind, now);

        let offer_recovery = !from_away && self.presence == Presence::Active;
        if offer_recovery {
            self.recovery_suggestions = self.pick_recovery();
            self.show_recovery_hint = true;
        } else {
            self.show_recovery_hint = false;
            self.recovery_suggestions.clear();
        }

        self.session = Some(Session {
            phase,
            planned_end: now + duration,
            paused_remaining: None,
            snooze_until: None,
        });
        self.sync_overrun(now);
    }

    /// Dismiss 恢复提示 without ending the Break.
    pub fn dismiss_recovery_hint(&mut self) {
        self.show_recovery_hint = false;
        // Keep suggestions cleared so UI stays quiet.
        self.recovery_suggestions.clear();
    }

    pub fn pause(&mut self, now: Instant) {
        let Some(session) = &mut self.session else {
            return;
        };
        if session.paused_remaining.is_some() {
            return;
        }
        let remaining = if now < session.planned_end {
            session.planned_end - now
        } else {
            Duration::ZERO
        };
        session.paused_remaining = Some(remaining);
        self.sync_overrun(now);
    }

    pub fn resume(&mut self, now: Instant) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(remaining) = session.paused_remaining.take() else {
            return;
        };
        session.planned_end = now + remaining;
        self.sync_overrun(now);
    }

    /// 加时：同一时段，计划结束点后移。
    pub fn extend(&mut self, now: Instant, by: Duration) {
        let Some(session) = &mut self.session else {
            return;
        };
        if let Some(rem) = &mut session.paused_remaining {
            *rem += by;
        } else if now < session.planned_end {
            session.planned_end += by;
        } else {
            session.planned_end = now + by;
        }
        session.snooze_until = None;
        self.sync_overrun(now);
    }

    /// 延后：不延长时段，只推迟下一次轻触。
    pub fn snooze(&mut self, now: Instant) {
        let Some(session) = &mut self.session else {
            return;
        };
        session.snooze_until = Some(now + SNOOZE_DELAY);
        self.return_nudge_armed = false;
    }

    /// 跳过：Focus → 下一段 Focus（不写休息）；Break → 开始专注。
    pub fn skip(&mut self, now: Instant) {
        let Some(session) = &self.session else {
            return;
        };
        match session.phase {
            Phase::Focus => {
                self.focuses_completed_in_cycle += 1;
                if self.focuses_completed_in_cycle >= FOCUSES_PER_LONG_BREAK {
                    self.focuses_completed_in_cycle = 0;
                }
                self.start_focus(now);
            }
            Phase::ShortBreak | Phase::LongBreak => {
                self.start_focus(now);
            }
        }
    }

    /// Observe idle duration from the Presence adapter (no keystroke content).
    pub fn observe_idle(&mut self, now: Instant, idle: Duration) {
        self.last_idle = idle;
        if idle >= IDLE_AWAY {
            self.enter_away(now, Presence::Away);
            return;
        }
        if matches!(
            self.presence,
            Presence::Away | Presence::LockedSleeping
        ) && idle < IDLE_QUIET
        {
            self.enter_returned(now);
            return;
        }
        // idle < AWAY: stay Active (brief idle only silences 轻触 via last_idle).
        // Returned persists until the user acts (start focus / skip / etc.).
        if self.presence == Presence::Active || self.presence == Presence::Returned {
            self.sync_overrun(now);
        }
    }

    /// Sleep / lock bucket: immediate 离开.
    pub fn observe_sleep(&mut self, now: Instant) {
        self.enter_away(now, Presence::LockedSleeping);
    }

    /// Wake from sleep notification.
    pub fn observe_wake(&mut self, now: Instant) {
        if matches!(
            self.presence,
            Presence::Away | Presence::LockedSleeping
        ) {
            self.enter_returned(now);
        }
    }

    fn enter_away(&mut self, now: Instant, presence: Presence) {
        if matches!(
            self.presence,
            Presence::Away | Presence::LockedSleeping
        ) {
            // Already away: upgrade Quiet→Away already handled; if Focus still running, ensure break.
            self.ensure_away_break(now);
            self.presence = presence;
            self.sync_overrun(now);
            return;
        }

        let kind = match presence {
            Presence::LockedSleeping => IntervalKind::LockedSleeping,
            _ => IntervalKind::Away,
        };
        self.open_presence_kind(kind, now);
        self.presence = presence;
        self.show_recovery_hint = false;
        self.recovery_suggestions.clear();
        self.ensure_away_break(now);
        self.sync_overrun(now);
    }

    fn ensure_away_break(&mut self, now: Instant) {
        let Some(session) = &self.session else {
            return;
        };
        match session.phase {
            Phase::Focus => {
                self.start_break_inner(now, true);
            }
            Phase::ShortBreak | Phase::LongBreak => {
                // Same Break continues — no nested Break.
            }
        }
    }

    fn enter_returned(&mut self, now: Instant) {
        self.clear_presence_interval(now);
        self.presence = Presence::Returned;
        // Discard backlog: silence while away already dropped nudges.
        // At most one Start Focus 轻触 if Break already ended.
        let break_past = self
            .session
            .as_ref()
            .map(|s| {
                matches!(s.phase, Phase::ShortBreak | Phase::LongBreak)
                    && s.paused_remaining.is_none()
                    && now >= s.planned_end
            })
            .unwrap_or(false);
        self.return_nudge_armed = break_past;
        if let Some(session) = &mut self.session {
            session.snooze_until = None;
        }
        self.sync_overrun(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn starts_idle() {
        let core = RhythmCore::new();
        let snap = core.snapshot(t0());
        assert_eq!(snap.phase, None);
        assert_eq!(snap.remaining_ms, 0);
        assert!(!snap.paused);
        assert!(!snap.should_nudge);
        assert_eq!(snap.presence, Presence::Active);
        assert!(!snap.show_recovery_hint);
    }

    #[test]
    fn start_focus_sets_phase_and_remaining_from_timestamps() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let snap = core.snapshot(now);
        assert_eq!(snap.phase, Some(Phase::Focus));
        assert_eq!(snap.remaining_ms, FOCUS_DURATION.as_millis() as u64);

        let later = now + Duration::from_secs(60);
        let snap = core.snapshot(later);
        assert_eq!(
            snap.remaining_ms,
            (FOCUS_DURATION - Duration::from_secs(60)).as_millis() as u64
        );
        assert!(!snap.should_nudge);
    }

    #[test]
    fn focus_then_short_break_then_focus() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.start_break(now + Duration::from_secs(1));
        assert_eq!(
            core.snapshot(now + Duration::from_secs(1)).phase,
            Some(Phase::ShortBreak)
        );
        assert_eq!(
            core.snapshot(now + Duration::from_secs(1)).remaining_ms,
            SHORT_BREAK_DURATION.as_millis() as u64
        );
        assert_eq!(
            core.snapshot(now + Duration::from_secs(1))
                .focuses_completed_in_cycle,
            1
        );

        core.start_focus(now + Duration::from_secs(2));
        assert_eq!(
            core.snapshot(now + Duration::from_secs(2)).phase,
            Some(Phase::Focus)
        );
    }

    #[test]
    fn fourth_focus_leads_to_long_break() {
        let mut core = RhythmCore::new();
        let mut now = t0();
        for i in 1..=4 {
            core.start_focus(now);
            now += Duration::from_secs(1);
            core.start_break(now);
            let snap = core.snapshot(now);
            if i < 4 {
                assert_eq!(snap.phase, Some(Phase::ShortBreak), "break after focus {i}");
            } else {
                assert_eq!(snap.phase, Some(Phase::LongBreak));
                assert_eq!(snap.remaining_ms, LONG_BREAK_DURATION.as_millis() as u64);
                assert_eq!(snap.focuses_completed_in_cycle, 0);
            }
            now += Duration::from_secs(1);
        }
    }

    #[test]
    fn pause_freezes_remaining_resume_recomputes_end() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let at_pause = now + Duration::from_secs(100);
        core.pause(at_pause);
        let frozen = core.snapshot(at_pause);
        assert!(frozen.paused);
        assert_eq!(
            frozen.remaining_ms,
            (FOCUS_DURATION - Duration::from_secs(100)).as_millis() as u64
        );

        let later = at_pause + Duration::from_secs(500);
        assert_eq!(core.snapshot(later).remaining_ms, frozen.remaining_ms);

        core.resume(later);
        let after = core.snapshot(later);
        assert!(!after.paused);
        assert_eq!(after.remaining_ms, frozen.remaining_ms);

        let after_one_min = later + Duration::from_secs(60);
        assert_eq!(
            core.snapshot(after_one_min).remaining_ms,
            frozen.remaining_ms - 60_000
        );
    }

    #[test]
    fn past_planned_end_reports_overrun_not_negative_remaining() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let past = now + FOCUS_DURATION + Duration::from_secs(30);
        let snap = core.snapshot(past);
        assert_eq!(snap.remaining_ms, 0);
        assert_eq!(snap.overrun_ms, 30_000);
        assert_eq!(snap.phase, Some(Phase::Focus));
        assert!(snap.should_nudge);
    }

    #[test]
    fn extend_moves_planned_end_on_same_session() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.extend(now, EXTEND_FIVE);
        assert_eq!(
            core.snapshot(now).remaining_ms,
            (FOCUS_DURATION + EXTEND_FIVE).as_millis() as u64
        );
        core.extend(now, EXTEND_TEN);
        assert_eq!(
            core.snapshot(now).remaining_ms,
            (FOCUS_DURATION + EXTEND_FIVE + EXTEND_TEN).as_millis() as u64
        );
        assert_eq!(core.snapshot(now).phase, Some(Phase::Focus));
    }

    #[test]
    fn extend_from_overrun_gives_fresh_remaining() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let past = now + FOCUS_DURATION + Duration::from_secs(60);
        assert!(core.snapshot(past).should_nudge);
        core.extend(past, EXTEND_FIVE);
        let snap = core.snapshot(past);
        assert_eq!(snap.remaining_ms, EXTEND_FIVE.as_millis() as u64);
        assert_eq!(snap.overrun_ms, 0);
        assert!(!snap.should_nudge);
    }

    #[test]
    fn snooze_defers_nudge_without_extending_session() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let past = now + FOCUS_DURATION + Duration::from_secs(10);
        assert_eq!(core.snapshot(past).overrun_ms, 10_000);
        assert!(core.snapshot(past).should_nudge);

        core.snooze(past);
        assert!(!core.snapshot(past).should_nudge);
        assert_eq!(core.snapshot(past).overrun_ms, 10_000);

        let still_snoozed = past + Duration::from_secs(60);
        assert!(!core.snapshot(still_snoozed).should_nudge);
        assert!(core.snapshot(still_snoozed).overrun_ms > 10_000);

        let after_snooze = past + SNOOZE_DELAY;
        assert!(core.snapshot(after_snooze).should_nudge);
    }

    #[test]
    fn skip_focus_starts_next_focus_without_break() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.skip(now + Duration::from_secs(1));
        let snap = core.snapshot(now + Duration::from_secs(1));
        assert_eq!(snap.phase, Some(Phase::Focus));
        assert_eq!(snap.focuses_completed_in_cycle, 1);
        assert_eq!(snap.remaining_ms, FOCUS_DURATION.as_millis() as u64);
    }

    #[test]
    fn skip_break_starts_focus() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.start_break(now + Duration::from_secs(1));
        core.skip(now + Duration::from_secs(2));
        assert_eq!(
            core.snapshot(now + Duration::from_secs(2)).phase,
            Some(Phase::Focus)
        );
    }

    #[test]
    fn ignoring_nudge_is_not_a_failure_state() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let past = now + FOCUS_DURATION + Duration::from_secs(5);
        let a = core.snapshot(past);
        let b = core.snapshot(past + Duration::from_secs(30));
        assert!(a.should_nudge && b.should_nudge);
        assert_eq!(a.phase, b.phase);
        assert_eq!(b.phase, Some(Phase::Focus));
    }

    #[test]
    fn quiet_idle_silences_nudge_but_keeps_focus() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let past = now + FOCUS_DURATION + Duration::from_secs(5);
        assert!(core.snapshot(past).should_nudge);

        core.observe_idle(past, IDLE_QUIET);
        let snap = core.snapshot(past);
        assert_eq!(snap.presence, Presence::Active);
        assert_eq!(snap.phase, Some(Phase::Focus));
        assert!(!snap.should_nudge);
    }

    #[test]
    fn five_minute_idle_equals_start_break() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let away_at = now + Duration::from_secs(60);
        core.observe_idle(away_at, IDLE_AWAY);
        let snap = core.snapshot(away_at);
        assert_eq!(snap.presence, Presence::Away);
        assert_eq!(snap.phase, Some(Phase::ShortBreak));
        assert!(!snap.show_recovery_hint);
    }

    #[test]
    fn away_during_break_does_not_nest_break() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.start_break(now + Duration::from_secs(1));
        let mid = now + Duration::from_secs(2);
        core.observe_idle(mid, IDLE_AWAY);
        assert_eq!(core.snapshot(mid).phase, Some(Phase::ShortBreak));
        assert_eq!(core.snapshot(mid).presence, Presence::Away);
    }

    #[test]
    fn leave_rules_apply_while_paused() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.pause(now + Duration::from_secs(10));
        let away_at = now + Duration::from_secs(20);
        core.observe_idle(away_at, IDLE_AWAY);
        assert_eq!(
            core.snapshot(away_at).phase,
            Some(Phase::ShortBreak)
        );
    }

    #[test]
    fn sleep_is_immediate_away() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.observe_sleep(now + Duration::from_secs(1));
        let snap = core.snapshot(now + Duration::from_secs(1));
        assert_eq!(snap.presence, Presence::LockedSleeping);
        assert_eq!(snap.phase, Some(Phase::ShortBreak));
    }

    #[test]
    fn return_after_break_ended_arms_single_focus_nudge() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.observe_idle(now + Duration::from_secs(1), IDLE_AWAY);
        let after_break = now + Duration::from_secs(1) + SHORT_BREAK_DURATION + Duration::from_secs(30);
        assert!(!core.snapshot(after_break).should_nudge);

        core.observe_idle(after_break, Duration::from_secs(0));
        let snap = core.snapshot(after_break);
        assert_eq!(snap.presence, Presence::Returned);
        assert!(snap.should_nudge);
        assert_eq!(snap.phase, Some(Phase::ShortBreak));

        // Returned holds across further active idle polls until the user acts.
        core.observe_idle(after_break + Duration::from_secs(1), Duration::ZERO);
        assert_eq!(
            core.snapshot(after_break + Duration::from_secs(1)).presence,
            Presence::Returned
        );
        assert!(core.snapshot(after_break + Duration::from_secs(1)).should_nudge);
    }

    #[test]
    fn intentional_break_shows_recovery_hint_once() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.start_break(now + Duration::from_secs(1));
        let snap = core.snapshot(now + Duration::from_secs(1));
        assert!(snap.show_recovery_hint);
        assert!(!snap.recovery_suggestions.is_empty());
        assert!(snap.recovery_suggestions.len() <= 3);
        for s in &snap.recovery_suggestions {
            assert!(RECOVERY_POOL.contains(&s.as_str()));
        }
    }

    #[test]
    fn dismiss_recovery_does_not_end_break() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.start_break(now + Duration::from_secs(1));
        core.dismiss_recovery_hint();
        let snap = core.snapshot(now + Duration::from_secs(1));
        assert!(!snap.show_recovery_hint);
        assert_eq!(snap.phase, Some(Phase::ShortBreak));
    }

    #[test]
    fn away_break_skips_recovery_hint() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.observe_idle(now + Duration::from_secs(1), IDLE_AWAY);
        assert!(!core.snapshot(now + Duration::from_secs(1)).show_recovery_hint);
    }

    #[test]
    fn closing_focus_emits_history_intervals() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let mid = now + Duration::from_secs(60);
        core.start_break(mid);
        let closed = core.drain_intervals();
        assert!(
            closed.iter().any(|i| i.kind == IntervalKind::Focus && i.start == now && i.end == mid),
            "expected closed focus interval, got {closed:?}"
        );
        assert!(closed.iter().any(|i| {
            matches!(i.kind, IntervalKind::ShortBreak) && i.start == mid
        }) == false); // ShortBreak still open
        // Open break is not drained until closed.
        core.start_focus(mid + Duration::from_secs(1));
        let closed2 = core.drain_intervals();
        assert!(closed2
            .iter()
            .any(|i| i.kind == IntervalKind::ShortBreak));
    }

    #[test]
    fn away_emits_away_interval() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        let away_at = now + Duration::from_secs(1);
        core.observe_idle(away_at, IDLE_AWAY);
        core.observe_idle(away_at + Duration::from_secs(10), Duration::ZERO);
        let closed = core.drain_intervals();
        assert!(closed.iter().any(|i| i.kind == IntervalKind::Away));
    }
}
