import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Phase = "focus" | "short_break" | "long_break";

type Snapshot = {
  phase: Phase | null;
  remaining_ms: number;
  overrun_ms: number;
  paused: boolean;
  focuses_completed_in_cycle: number;
};

const PHASE_LABEL: Record<Phase, string> = {
  focus: "Focus",
  short_break: "Short Break",
  long_break: "Long Break",
};

function formatClock(ms: number, prefix = ""): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${prefix}${m}:${s.toString().padStart(2, "0")}`;
}

function formatRemaining(ms: number, overrunMs: number): string {
  if (overrunMs > 0) {
    return formatClock(overrunMs, "+");
  }
  return formatClock(ms);
}

function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);

  const refresh = useCallback(async () => {
    setSnapshot(await invoke<Snapshot>("rhythm_snapshot"));
  }, []);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => {
      void refresh();
    }, 250);
    return () => window.clearInterval(id);
  }, [refresh]);

  async function startFocus() {
    setSnapshot(await invoke<Snapshot>("rhythm_start_focus"));
  }

  async function startBreak() {
    setSnapshot(await invoke<Snapshot>("rhythm_start_break"));
  }

  async function pause() {
    setSnapshot(await invoke<Snapshot>("rhythm_pause"));
  }

  async function resume() {
    setSnapshot(await invoke<Snapshot>("rhythm_resume"));
  }

  if (!snapshot) {
    return <main className="shell">Loading…</main>;
  }

  const phase = snapshot.phase;
  const idle = phase === null;
  const inFocus = phase === "focus";
  const inBreak = phase === "short_break" || phase === "long_break";

  return (
    <main className="shell">
      <p className="phase">{phase ? PHASE_LABEL[phase] : "Ready"}</p>
      <p className="time" aria-live="polite">
        {formatRemaining(snapshot.remaining_ms, snapshot.overrun_ms)}
      </p>
      {snapshot.paused ? <p className="hint">Paused</p> : null}

      <div className="actions">
        {idle || inBreak ? (
          <button type="button" className="primary" onClick={() => void startFocus()}>
            Start Focus
          </button>
        ) : null}
        {inFocus ? (
          <button type="button" className="primary" onClick={() => void startBreak()}>
            Start Break
          </button>
        ) : null}
        {!idle && !snapshot.paused ? (
          <button type="button" className="quiet" onClick={() => void pause()}>
            Pause
          </button>
        ) : null}
        {snapshot.paused ? (
          <button type="button" className="quiet" onClick={() => void resume()}>
            Resume
          </button>
        ) : null}
      </div>
    </main>
  );
}

export default App;
