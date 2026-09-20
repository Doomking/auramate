import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./Pet.css";

type PetState = "idle" | "nudge" | "rest";

type Snapshot = {
  pet_state: PetState;
};

const LABEL: Record<PetState, string> = {
  idle: "…",
  nudge: "!",
  rest: "z",
};

export default function Pet() {
  const [state, setState] = useState<PetState>("idle");

  const refresh = useCallback(async () => {
    const snap = await invoke<Snapshot>("rhythm_snapshot");
    setState(snap.pet_state);
  }, []);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => {
      void refresh();
    }, 250);
    return () => window.clearInterval(id);
  }, [refresh]);

  return (
    <div className={`pet pet-${state}`} role="img" aria-label={`Pet ${state}`}>
      <div className="pet-body" />
      <span className="pet-mark">{LABEL[state]}</span>
    </div>
  );
}
