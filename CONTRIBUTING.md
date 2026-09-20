# Contributing

Thanks for helping with AuraMate (灵息).

## Before you code

1. Read `docs/requirement.md` (product intent) and `docs/spec.md` (phase 1 plan).
2. Use domain words from `CONTEXT.md` — do not invent synonyms for 阶段 / 时段 / 轻触 / 存在.
3. Prefer picking an open GitHub issue labelled `ready-for-agent` whose blockers are closed. See **What's next** in `AGENTS.md`.

## Workflow

1. Open or claim an issue.
2. Branch from `main` (e.g. `implement/<n>-short-slug`).
3. Keep changes scoped to that issue. Do not add gamification or phase 2 AI.
4. Run `pnpm check` and `cd src-tauri && cargo test` before opening a PR.
5. Open a PR against `main` and link the issue.

## Privacy

Do not add telemetry, screen/keystroke capture, or detailed app-usage logging. Presence stays coarse and local by default.
