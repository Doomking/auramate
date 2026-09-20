# AGENTS.md

Instructions for coding agents working in this repository.

## What this is

**AuraMate** (灵息) is a local-first desktop companion for work–rest rhythm. Pomodoro is the mechanism; the product is a gentle tap, not an interruption.

Humans read `README.md`. Product intent lives in `docs/requirement.md`. Phase 1 spec: `docs/spec.md`. Read those before changing UX, presence behaviour, pet behaviour, or roadmap scope.

## Commands

Scaffolded with official `create-tauri-app` (Tauri 2, SvelteKit + TypeScript, pnpm).

| Action | Command |
| --- | --- |
| Install JS deps | `pnpm install` |
| Desktop dev | `pnpm tauri dev` |
| Frontend only | `pnpm dev` |
| Typecheck (Svelte/TS) | `pnpm check` |
| Lint (scaffold baseline) | `pnpm check`（模板未带 eslint；类型检查即当前 lint 门禁） |
| Frontend production build | `pnpm build` |
| Desktop production build | `pnpm tauri build` |
| Rust checks (crate) | `cd src-tauri && cargo check` |
| Rust tests (crate) | `cd src-tauri && cargo test` |

Rhythm Core tests will live in the Rust crate; prefer `cargo test` at the Rhythm Core seam once that module exists (`docs/spec.md`).

## Stack (locked)

| Layer | Choice |
| --- | --- |
| Desktop | Rust + Tauri 2 |
| UI | SvelteKit + TypeScript (from `create-tauri-app` `svelte-ts` template) |
| Domain logic | Rust owns Timer, Session, Reminder, Presence, and remaining-time math |
| Storage | SQLite for session, break, overrun, and presence intervals |
| Time | Remaining time from real timestamps, not `setInterval` decrement |

## Product constraints

Treat these as invariants, not suggestions:

- **Ambient first.** Reminders change environment (tray, pet, corner cue) before system notifications. Never steal focus, go fullscreen, or force-block the current task.
- **Advice, not failure.** Snooze, Skip, and +5 / +10 minutes are first-class. Ignoring a nudge is not a loss state.
- **Observe, then nudge.** On a Focus boundary, classify Presence before prompting. Away / Locked / Sleeping means stop or reduce prompts; Returned resumes from context instead of flushing a backlog.
- **Coarse presence only.** Phase 1 states: Active, Media/Meeting, Away, Locked/Sleeping, Returned. Presence exists to avoid pointless reminders, not to score productivity.
- **Pets are interaction language.** Idle, quiet, low-frequency. They are not characters to feed, level, dress, or quest.
- **Local-first.** Timer, Presence, and history work offline. Presence data stays on device by default. No activity telemetry unless the user later opts in.

## Phase 1 scope

Build the MVP in `docs/requirement.md` §5 and §10 and `docs/spec.md`: Focus / Short Break / Long Break, ambient nudge, Snooze / +5 / +10, 2–3 pets, tray, local history.

Leave phase 2 (pluggable AI) and phase 3 (richer companion) until the MVP loop is real. Do not add gamification (XP, coins, streaks, check-ins, social, forced focus).

## Privacy and secrets

- Do not capture screen contents, keystrokes, page contents, mouse trails, or detailed app-usage history.
- API keys (phase 2) go in the OS keychain, not repo files or plaintext config.
- Third-party art, fonts, and audio must record source, license, and redistribution terms.

## Visual tone

Quiet, warm, sparse. Time is the primary visual. One primary action plus at most one or two light actions. Motion is a state change (appear / hold / fade), not a reward.

## Agent skills

### Issue tracker

GitHub Issues on `Doomking/auramate`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default roles: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` plus `docs/adr/`. See `docs/agents/domain.md`.
