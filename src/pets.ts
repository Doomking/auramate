/** Pet kinds and still paths — expression only; rules live in Rhythm Core. */

export const PET_KINDS = ["cat", "dog", "plant"] as const;
export type PetKind = (typeof PET_KINDS)[number];

export type PetState = "idle" | "nudge" | "rest";

export const PET_KIND_STORAGE_KEY = "auramate.petKind";
/** Same-window signal when the main UI changes pet kind (storage events only cross windows). */
export const PET_KIND_CHANGE_EVENT = "auramate-pet-kind";

const PET_KIND_LABEL: Record<PetKind, string> = {
  cat: "Cat",
  dog: "Dog",
  plant: "Plant",
};

export function petKindLabel(kind: PetKind): string {
  return PET_KIND_LABEL[kind];
}

/** Public URL for a pet still (nine assets under `public/pets/`). */
export function petStillSrc(kind: PetKind, state: PetState): string {
  return `/pets/${kind}-${state}.jpg`;
}

export function isPetKind(value: string): value is PetKind {
  return (PET_KINDS as readonly string[]).includes(value);
}

export function readPetKind(): PetKind {
  try {
    const raw = localStorage.getItem(PET_KIND_STORAGE_KEY);
    if (raw && isPetKind(raw)) {
      return raw;
    }
  } catch {
    // ignore (private mode / unavailable storage)
  }
  return "cat";
}

export function writePetKind(kind: PetKind): void {
  try {
    localStorage.setItem(PET_KIND_STORAGE_KEY, kind);
  } catch {
    // ignore
  }
}
