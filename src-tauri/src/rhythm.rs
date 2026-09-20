//! Rhythm Core: Focus / Short Break / Long Break with timestamp-based remaining time.
//! Public seam for phase-1 session behaviour (see `docs/spec.md`).

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// Current 阶段, or `null` when no 时段 is running.
    pub phase: Option<Phase>,
    /// Milliseconds until planned end. `0` when idle, paused uses frozen value; past end stays `0`.
    pub remaining_ms: u64,
    /// Milliseconds past planned end while still in the 时段 (超时).
    pub overrun_ms: u64,
    pub paused: bool,
    /// Completed Focus 时段s in the current cycle (0..FOCUSES_PER_LONG_BREAK).
    pub focuses_completed_in_cycle: u32,
}

#[derive(Debug, Clone)]
struct Session {
    phase: Phase,
    planned_end: Instant,
    paused_remaining: Option<Duration>,
}

/// Deep module owning 阶段 / 时段 / 暂停 semantics.
#[derive(Debug, Clone)]
pub struct RhythmCore {
    session: Option<Session>,
    focuses_completed_in_cycle: u32,
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

        Snapshot {
            phase: Some(session.phase),
            remaining_ms: remaining.as_millis() as u64,
            overrun_ms: overrun.as_millis() as u64,
            paused,
            focuses_completed_in_cycle: self.focuses_completed_in_cycle,
        }
    }

    /// Start a Focus 时段 from Idle or after a Break.
    pub fn start_focus(&mut self, now: Instant) {
        self.session = Some(Session {
            phase: Phase::Focus,
            planned_end: now + FOCUS_DURATION,
            paused_remaining: None,
        });
    }

    /// End Focus and start Short or Long Break (after every fourth Focus).
    pub fn start_break(&mut self, now: Instant) {
        let Some(session) = &self.session else {
            return;
        };
        if session.phase != Phase::Focus {
            return;
        }

        self.focuses_completed_in_cycle += 1;
        let (phase, duration) = if self.focuses_completed_in_cycle >= FOCUSES_PER_LONG_BREAK {
            self.focuses_completed_in_cycle = 0;
            (Phase::LongBreak, LONG_BREAK_DURATION)
        } else {
            (Phase::ShortBreak, SHORT_BREAK_DURATION)
        };

        self.session = Some(Session {
            phase,
            planned_end: now + duration,
            paused_remaining: None,
        });
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
    }

    pub fn resume(&mut self, now: Instant) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(remaining) = session.paused_remaining.take() else {
            return;
        };
        session.planned_end = now + remaining;
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
    }

    #[test]
    fn focus_then_short_break_then_focus() {
        let mut core = RhythmCore::new();
        let now = t0();
        core.start_focus(now);
        core.start_break(now + Duration::from_secs(1));
        assert_eq!(core.snapshot(now + Duration::from_secs(1)).phase, Some(Phase::ShortBreak));
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

        // Time passes while paused — remaining unchanged
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
    }
}
