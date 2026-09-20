//! Quit/relaunch checkpoint adapter — durable SessionCheckpoint + wall quit time.
//! Rhythm Core owns the short/long gap policy; this file only loads and saves JSON.

use crate::rhythm::SessionCheckpoint;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedCheckpoint {
    pub quit_wall_ms: i64,
    pub checkpoint: SessionCheckpoint,
}

pub struct CheckpointStore {
    path: PathBuf,
}

impl CheckpointStore {
    pub fn open(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        Self { path }
    }

    pub fn load(&self) -> Option<PersistedCheckpoint> {
        let bytes = fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn save(&self, data: Option<&PersistedCheckpoint>) -> std::io::Result<()> {
        match data {
            Some(data) => {
                let bytes = serde_json::to_vec_pretty(data)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                fs::write(&self.path, bytes)
            }
            None => {
                if self.path.exists() {
                    fs::remove_file(&self.path)?;
                }
                Ok(())
            }
        }
    }
}

pub fn wall_ms(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as i64
}

pub fn gap_since_quit(quit_wall_ms: i64, wall_now: SystemTime) -> Duration {
    let now_ms = wall_ms(wall_now);
    if now_ms <= quit_wall_ms {
        return Duration::ZERO;
    }
    Duration::from_millis((now_ms - quit_wall_ms) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhythm::{IntervalKind, Phase, RhythmCore, FOCUS_DURATION, IDLE_AWAY};
    use std::time::Instant;

    #[test]
    fn round_trip_json_and_short_gap_restore() {
        let dir = std::env::temp_dir().join(format!(
            "auramate-checkpoint-{}-{}",
            std::process::id(),
            wall_ms(SystemTime::now())
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("session.json");
        let store = CheckpointStore::open(path);

        let now = Instant::now();
        let mut core = RhythmCore::new();
        core.start_focus(now);
        let quit = now + Duration::from_secs(60);
        let cp = core.export_checkpoint(quit).expect("cp");
        let quit_wall = SystemTime::now();
        store
            .save(Some(&PersistedCheckpoint {
                quit_wall_ms: wall_ms(quit_wall),
                checkpoint: cp.clone(),
            }))
            .expect("save");

        let loaded = store.load().expect("load");
        assert_eq!(loaded.checkpoint, cp);
        assert_eq!(loaded.checkpoint.session_kind, IntervalKind::Focus);

        let relaunch_wall = quit_wall + Duration::from_secs(90);
        let gap = gap_since_quit(loaded.quit_wall_ms, relaunch_wall);
        assert_eq!(gap, Duration::from_secs(90));
        assert!(gap < IDLE_AWAY);

        let mut restored = RhythmCore::new();
        let relaunch = quit + gap;
        restored.relaunch_from_checkpoint(loaded.checkpoint, gap, relaunch);
        let snap = restored.snapshot(relaunch);
        assert_eq!(snap.phase, Some(Phase::Focus));
        assert_eq!(
            snap.remaining_ms,
            (FOCUS_DURATION - Duration::from_secs(150)).as_millis() as u64
        );

        store.save(None).expect("clear");
        assert!(store.load().is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
