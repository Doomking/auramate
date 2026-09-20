import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  PET_KIND_CHANGE_EVENT,
  petStillSrc,
  readPetKind,
  type PetKind,
  type PetState,
} from "./pets";
import "./Pet.css";

type Snapshot = {
  pet_state: PetState;
};

export default function Pet() {
  const [state, setState] = useState<PetState>("idle");
  const [kind, setKind] = useState<PetKind>(() => readPetKind());

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

  useEffect(() => {
    const syncKind = () => setKind(readPetKind());
    window.addEventListener("storage", syncKind);
    window.addEventListener(PET_KIND_CHANGE_EVENT, syncKind);
    const id = window.setInterval(syncKind, 500);
    return () => {
      window.removeEventListener("storage", syncKind);
      window.removeEventListener(PET_KIND_CHANGE_EVENT, syncKind);
      window.clearInterval(id);
    };
  }, []);

  return (
    <div className={`pet pet-${state}`} role="img" aria-label={`${kind} ${state}`}>
      <img
        className="pet-still"
        src={petStillSrc(kind, state)}
        alt=""
        draggable={false}
      />
    </div>
  );
}
