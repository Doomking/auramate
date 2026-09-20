import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Phase = "focus" | "short_break" | "long_break";
type Presence = "active" | "away" | "locked_sleeping" | "returned";

type Snapshot = {
  phase: Phase | null;
  remaining_ms: number;
  overrun_ms: number;
  paused: boolean;
  focuses_completed_in_cycle: number;
  should_nudge: boolean;
  presence: Presence;
  show_recovery_hint: boolean;
  recovery_suggestions: string[];
};

type HistoryRow = {
  id: number;
  kind: string;
  start_ms: number;
  end_ms: number;
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

function formatWhen(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatKind(kind: string): string {
  return kind.replace(/_/g, " ");
}

function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [view, setView] = useState<"main" | "history">("main");
  const [history, setHistory] = useState<HistoryRow[]>([]);

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

  async function call(cmd: string, args?: Record<string, unknown>) {
    setSnapshot(await invoke<Snapshot>(cmd, args));
  }

  async function openHistory() {
    const rows = await invoke<HistoryRow[]>("history_list", { limit: 100 });
    setHistory(rows);
    setView("history");
  }

  if (!snapshot) {
    return <main className="shell">Loading…</main>;
  }

  if (view === "history") {
    return (
      <main className="shell history-shell">
        <header className="history-header">
          <button type="button" className="quiet" onClick={() => setView("main")}>
            Back
          </button>
          <h1 className="history-title">History</h1>
        </header>
        {history.length === 0 ? (
          <p className="hint">No intervals yet</p>
        ) : (
          <ul className="history-list">
            {history.map((row) => (
              <li key={row.id} className="history-row">
                <span className="history-kind">{formatKind(row.kind)}</span>
                <span className="history-when">
                  {formatWhen(row.start_ms)} – {formatWhen(row.end_ms)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </main>
    );
  }

  const phase = snapshot.phase;
  const idle = phase === null;
  const inFocus = phase === "focus";
  const inBreak = phase === "short_break" || phase === "long_break";
  const active = !idle;

  return (
    <main className="shell">
      <p className="phase">{phase ? PHASE_LABEL[phase] : "Ready"}</p>
      <p className="time" aria-live="polite">
        {formatRemaining(snapshot.remaining_ms, snapshot.overrun_ms)}
      </p>
      {snapshot.paused ? <p className="hint">Paused</p> : null}
      {snapshot.should_nudge ? (
        <p className="hint nudge" role="status">
          Gentle nudge — time for the next step
        </p>
      ) : null}

      {snapshot.show_recovery_hint ? (
        <aside className="recovery" aria-label="Recovery suggestions">
          <p className="recovery-title">While you rest</p>
          <ul>
            {snapshot.recovery_suggestions.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
          <button
            type="button"
            className="quiet"
            onClick={() => void call("rhythm_dismiss_recovery")}
          >
            Dismiss
          </button>
        </aside>
      ) : null}

      <div className="actions">
        {idle || inBreak ? (
          <button type="button" className="primary" onClick={() => void call("rhythm_start_focus")}>
            Start Focus
          </button>
        ) : null}
        {inFocus ? (
          <button type="button" className="primary" onClick={() => void call("rhythm_start_break")}>
            Start Break
          </button>
        ) : null}
        {active && !snapshot.paused ? (
          <button type="button" className="quiet" onClick={() => void call("rhythm_pause")}>
            Pause
          </button>
        ) : null}
        {snapshot.paused ? (
          <button type="button" className="quiet" onClick={() => void call("rhythm_resume")}>
            Resume
          </button>
        ) : null}
      </div>

      {active ? (
        <div className="actions secondary">
          <button type="button" className="quiet" onClick={() => void call("rhythm_extend", { minutes: 5 })}>
            +5
          </button>
          <button type="button" className="quiet" onClick={() => void call("rhythm_extend", { minutes: 10 })}>
            +10
          </button>
          <button type="button" className="quiet" onClick={() => void call("rhythm_snooze")}>
            Snooze
          </button>
          <button type="button" className="quiet" onClick={() => void call("rhythm_skip")}>
            Skip
          </button>
        </div>
      ) : null}

      <nav className="footer-nav">
        <button type="button" className="quiet linkish" onClick={() => void openHistory()}>
          History
        </button>
      </nav>
    </main>
  );
}

export default App;
