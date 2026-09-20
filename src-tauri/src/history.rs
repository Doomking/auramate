//! Local history adapter — persists Rhythm Core interval intents to SQLite.
//! No upload path; no scores or streaks.

use crate::rhythm::{ClosedInterval, IntervalKind};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct HistoryRow {
    pub id: i64,
    pub kind: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

pub struct HistoryStore {
    conn: Connection,
}

impl HistoryStore {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS intervals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                start_ms INTEGER NOT NULL,
                end_ms INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_intervals_start ON intervals(start_ms DESC);",
        )?;
        Ok(Self { conn })
    }

    pub fn append_closed(
        &self,
        intervals: &[ClosedInterval],
        instant_now: Instant,
        wall_now: SystemTime,
    ) -> rusqlite::Result<()> {
        let wall_now_ms = system_time_ms(wall_now);
        let mut stmt = self.conn.prepare(
            "INSERT INTO intervals (kind, start_ms, end_ms) VALUES (?1, ?2, ?3)",
        )?;
        for interval in intervals {
            let start_ms = wall_now_ms
                - duration_ms(instant_now.saturating_duration_since(interval.start)) as i64;
            let end_ms = wall_now_ms
                - duration_ms(instant_now.saturating_duration_since(interval.end)) as i64;
            stmt.execute(params![kind_label(interval.kind), start_ms, end_ms])?;
        }
        Ok(())
    }

    pub fn list_recent(&self, limit: usize) -> rusqlite::Result<Vec<HistoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, start_ms, end_ms FROM intervals ORDER BY start_ms DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(HistoryRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                start_ms: row.get(2)?,
                end_ms: row.get(3)?,
            })
        })?;
        rows.collect()
    }
}

fn kind_label(kind: IntervalKind) -> &'static str {
    match kind {
        IntervalKind::Focus => "focus",
        IntervalKind::ShortBreak => "short_break",
        IntervalKind::LongBreak => "long_break",
        IntervalKind::Overrun => "overrun",
        IntervalKind::Away => "away",
        IntervalKind::LockedSleeping => "locked_sleeping",
        IntervalKind::HyperFocus => "hyper_focus",
        IntervalKind::MediaMeeting => "media_meeting",
    }
}

fn system_time_ms(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as i64
}

fn duration_ms(d: Duration) -> u128 {
    d.as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhythm::IntervalKind;
    use std::time::Duration;

    #[test]
    fn append_and_list_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "auramate-history-{}-{}",
            std::process::id(),
            system_time_ms(SystemTime::now())
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("history.sqlite");
        let store = HistoryStore::open(&path).expect("open store");
        let now = Instant::now();
        let wall = SystemTime::now();
        let intervals = vec![ClosedInterval {
            kind: IntervalKind::Focus,
            start: now - Duration::from_secs(60),
            end: now,
        }];
        store
            .append_closed(&intervals, now, wall)
            .expect("append");
        let rows = store.list_recent(10).expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "focus");
        assert!(rows[0].end_ms >= rows[0].start_ms);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
