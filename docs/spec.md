# 灵息 · AuraMate — 一期规格

本规格由 [一期可交接规格](https://github.com/Doomking/auramate/issues/1) 上的决策收成。细节以各决策票为准；此处只链到票，不把票上的答案抄第二份。背景意图见 `docs/requirement.md`；领域词汇见根目录 `CONTEXT.md`。

**主测试接缝：节奏核心（Rhythm Core）**（Rust）。适配器：存在、历史（SQLite）、环境表面（托盘 / 宠物窗 / 主窗口）。

---

## Problem Statement

用户需要在保持专注的同时维持工作—休息节奏，但不想被番茄钟式工具抢焦点、弹窗打断，或被游戏化任务分心。离开电脑后仍被催促，或回来时被积压提醒淹没，都会让工具变成负担。一期要在本机提供一个安静的桌面伙伴：到点用环境变化轻触，结合粗粒度存在感知决定是否打扰，并把最终决定权留给用户。

## Solution

AuraMate 一期是一个本地优先的桌面应用（Tauri 2 + React + Rust）。用户开始一段专注时段后，主界面与托盘安静显示阶段与剩余时间；宠物低频待机。到点后托盘变化，宠物（若开启）做一次轻触；系统通知默认关闭。用户可开始休息、加时、延后、跳过或暂停——没有失败态。离开约五分钟等于开始休息；人回来只按当前时段接上，不冲刷积压。媒体/会议与深度心流在需要时推迟或停止宠物轻触。历史只在本机、藏在次级页。

## User Stories

1. As a knowledge worker, I want to start a Focus 时段 without the app stealing focus, so that I can keep working in my current tool.
2. As a user, I want to see the current 阶段 and remaining time at a glance in the main window, so that time is the primary signal.
3. As a user, I want Pause and Resume, so that I can freeze remaining time when I need a short interruption I control.
4. As a user, I want the tray / menu bar to show Focus / Break status, so that I know the rhythm without opening the main window.
5. As a user, I want to Pause, Skip, and Snooze from the tray, so that I need not bring the main window forward.
6. As a user, I want remaining time computed from real timestamps, so that sleep or backgrounding does not silently drift the clock.
7. As a user, I want Focus → Short Break → Focus cycling, with a Long Break after four Focus 时段, so that the classic rhythm works offline.
8. As a user, I want Start Break at a Focus boundary, so that I can enter rest when I choose.
9. As a user, I want Start Focus at a Break boundary, so that rest does not leave me stuck.
10. As a user, I want +5 and +10 加时 on the same 时段, so that I can keep working without starting a new session.
11. As a user, I want 延后 (Snooze) to postpone the next 提醒 without extending the 时段, so that I can say “not now” without pretending I got more Focus time.
12. As a user, I want Skip when a Break is due, so that I can go straight to the next Focus without recording a Break.
13. As a user, I want Skip during a Break, so that I can start Focus early.
14. As a user, I want ignoring a nudge to not be treated as failure, so that advice stays advice.
15. As a user, I want an ambient 轻触 (tray change + optional pet motion) at a boundary, so that I am informed without a modal interruption.
16. As a user, I want system notifications off by default, so that the OS notification center is not the primary channel.
17. As a user, I want an optional setting to enable system notifications, so that I can opt into a louder channel.
18. As a user, I want no automatic escalation to system notifications during 超时, so that dragging past the boundary stays quiet unless I opted in.
19. As a user, I want pet 轻触 during 超时 at most about every five minutes, so that the pet does not thrash.
20. As a user, I want no separate corner chrome; the pet is the corner presence, so that surfaces stay sparse.
21. As a user, I want the pet window never to steal focus, so that typing is not interrupted.
22. As a user, I want ~30 seconds without input to silence active 轻触 while Focus continues, so that brief stillness is not treated as leaving.
23. As a user, I want ~5 minutes without input to count as 离开 and equal Start Break, so that walking away becomes rest without a button.
24. As a user on Windows, I want lock and sleep to count as 离开 immediately, so that locking the machine stops prompts.
25. As a user on macOS, I want sleep to count as 离开 immediately via public notifications, so that sleeping the machine stops prompts.
26. As a user on macOS, I want lock without a public API to fall back to idle timing, so that we never require private lock hooks in phase 1.
27. As a user who left during Focus, I want a Break 时段 started automatically, so that absence is rest, not a failed Focus.
28. As a user already on Break who goes 离开, I want the same Break to continue, so that we do not spawn nested Breaks.
29. As a user who returns while Break is still running, I want the Break countdown to continue quietly, so that I am not nudged for being back mid-break.
30. As a user who returns after Break has ended, I want a single 轻触 offering Start Focus, so that I can resume without a backlog of reminders.
31. As a user who was only briefly idle (not full 离开), I want the same Focus 时段 to continue, so that short pauses do not reset context.
32. As a returning user, I want all missed 轻触 from while I was away discarded, so that I am never flushed with a backlog.
33. As a user in a call or watching media, I want 媒体/会议 to postpone pet 轻触 when detectable, so that ambient motion does not interrupt.
34. As a user, I want 媒体/会议 detected only via mic/camera busy or public DND/presentation-style signals, so that screen and keystroke contents are never captured.
35. As a user, I want meeting and media treated as one state in phase 1, so that we do not pretend we can tell them apart privately.
36. As a user whose OS cannot expose those signals, I want the app to run without 媒体/会议, so that missing signals are a graceful downgrade.
37. As a user who exits 媒体/会议 after a boundary passed, I want at most one catch-up 轻触, so that deferred prompts do not pile up.
38. As a user in deep work past the Focus boundary, I want 深度心流 after two unanswered 轻触 while still 活动, so that the pet stops nudging without scoring me as failed.
39. As a user in 深度心流, I want tray time to remain visible without urgent flashing, so that status is still ambient.
40. As a user in 深度心流, I want any button, 离开, or lock/sleep to exit that mode, so that I can re-enter normal advice.
41. As a user, I want 媒体/会议 periods not to count toward entering 深度心流, so that deferred nudges are not mistaken for ignored nudges.
42. As a user starting Break while present, I want one 恢复提示 with 1–3 fixed suggestions (water, walk, look away, etc.), so that recovery advice arrives once.
43. As a user, I want that card dismissible without ending Break, so that advice is optional.
44. As a user whose Break started because of 离开, I want no 恢复提示, so that the app does not advise an empty chair.
45. As a user, I want to choose among Cat, Dog, and Plant pets with identical rules, so that personality is expression only.
46. As a user, I want pet states Idle / 轻触 / Rest only, so that there is no failure face or quest state.
47. As a user, I want the pet on the edge of the monitor under the mouse pointer, moving only when I change displays, so that it stays in peripheral vision without chasing the cursor.
48. As a user, I want Focus-time pet motion to stay very low frequency, so that the companion stays quiet while I work.
49. As a user, I want local history of 时段, 超时, 深度心流, 离开, lock/sleep, and 媒体/会议 intervals, so that I can review rhythm later.
50. As a user, I want no productivity scores, streaks, or dashboards on the main window, so that history never becomes gamification.
51. As a user, I want history on a secondary page only, so that the primary surface stays about now.
52. As a user, I want history never uploaded by default, so that presence data stays on device.
53. As a user, I want Pause to freeze remaining time and resume by recomputing the end from now + remaining, so that pause is explicit and honest.
54. As a user, I want 离开 rules to still apply while Paused, so that walking away during pause becomes Break.
55. As a user, I want a 时段 that crosses midnight to stay one 时段, so that overnight work is not artificially split.
56. As a user who quits the app for under ~5 minutes, I want the same 时段 restored from timestamps, so that brief exits do not reset rhythm.
57. As a user who quits for ~5 minutes or more, I want that treated as 离开 (= Start Break) with Returned rules afterward, so that long downtime matches presence policy.
58. As a user on a machine without Accessibility permission, I want Focus / Break / tray / pet / idle-based 离开 to still work, so that core rhythm never depends on scary permission prompts.
59. As a user, I want phase 1 not to demand Accessibility at launch, so that first run stays gentle.
60. As a macOS and Windows user reading the spec, I want both platforms described, so that Windows is not designed out even if Mac ships first.
61. As a developer shipping phase 1, I want to implement Mac first while keeping the Windows contracts in the spec, so that porting is not a redesign.
62. As an open-source maintainer, I want pet art to be AI-generated under redistributable terms with credits recorded, so that shipping sprites does not violate licenses.
63. As a user offline, I want the full phase-1 loop to work, so that local-first is real.
64. As a privacy-conscious user, I want no screen contents, key contents, page contents, mouse trails, or detailed app-usage history collected, so that presence stays coarse.
65. As a user, I want visual tone from requirement §7 (warm off-white, moss accent, large time, appear/hold/fade motion), so that the UI stays quiet and non-gamey.

## Implementation Decisions

### Stack and layout

- Desktop: Rust + Tauri 2; UI: React + TypeScript (`create-tauri-app` `react-ts`); do not invent the tree by hand. See `AGENTS.md` and `docs/adr/0001-react-ui.md`.
- Domain ownership: Rust owns Timer / Session / Reminder / Presence and remaining-time math from timestamps, not `setInterval` decrement.
- Storage: SQLite for session, break, overrun, and presence-related intervals.
- Platforms: spec covers macOS and Windows; implement Mac first. Decision: [一期目标平台是只 macOS，还是 macOS + Windows？](https://github.com/Doomking/auramate/issues/3).
- Spec home: this file (`docs/spec.md`); requirement doc is background only. Decision: [一期总规格落在 docs/spec.md 还是一张 GitHub issue？](https://github.com/Doomking/auramate/issues/2).

### Primary seam: Rhythm Core

- One deep module in Rust: **Rhythm Core**. Interface accepts wall-clock time, user actions (Start Focus / Start Break / Extend +5|+10 / Snooze / Skip / Pause / Resume), and presence observations (idle duration, sleep/lock when known, optional media/meeting proxy). Interface returns current 时段 and remaining time, whether to 轻触, pet display state (Idle / 轻触 / Rest), whether to show 恢复提示, and which history intervals to append.
- Adapters (not primary test surface): Presence (OS idle / sleep / optional mic-camera or DND), History (SQLite), Ambient surfaces (tray, pet window, main window) that render core state without re-implementing policy.
- Facts on OS signals: [不采集内容的前提下，系统能提供哪些存在信号？](https://github.com/Doomking/auramate/issues/7) → `docs/research/presence-signals.md` on branch `research/presence-signals`.
- Facts on Tauri windows/tray: [Tauri 2 在非抢焦点窗、托盘、透明置顶上的能力边界是什么？](https://github.com/Doomking/auramate/issues/8) → `docs/research/tauri-ambient-windows.md` on branch `research/tauri-ambient-windows`. Pet/tray: `focus: false` + `focusable: false`; tray menus for Pause/Skip/Snooze; macOS transparency may need `macOSPrivateApi`; multi-monitor via monitor geometry + `setPosition`.

### Session actions and presence thresholds

- Action semantics: [Skip / Snooze / +5 / +10 / 开始休息各自如何改时段？](https://github.com/Doomking/auramate/issues/4).
- ~30s idle: silence 轻触, Focus continues; ~5 min idle: 离开 = Start Break; lock/sleep immediate 离开 where detectable: [离开如何判定；离开一段时间能否视为一次自然休息？](https://github.com/Doomking/auramate/issues/10).
- Permissions: no Accessibility ask at launch; core loop on idle + sleep notifications: [权限请求以及低权限空闲 API 降级是否一期必做？](https://github.com/Doomking/auramate/issues/11).
- Pause / midnight / relaunch: [暂停、跨午夜、关机后，时段如何接上？](https://github.com/Doomking/auramate/issues/18).

### Ambient nudge and surfaces

- Tray always updates at boundary; pet motion if enabled; no independent corner widget; system notifications default off and never auto-escalate: [到点后轻触的主通道是什么，何时才升到系统通知？](https://github.com/Doomking/auramate/issues/9).
- Pet follows pointer’s monitor edge only on display change: [宠物与角落提示是否跟随活动屏幕？](https://github.com/Doomking/auramate/issues/14).
- Pets Cat / Dog / Plant; states Idle / 轻触 / Rest only: [宠物一期有哪些可感知状态；2–3 只如何差异化、又不变成养成？](https://github.com/Doomking/auramate/issues/6).
- Art: maintainer AI-generated nine stills; redistributable tool only; record in `docs/credits.md`: [一期猫/狗/植物的原画从哪来，许可证如何记录？](https://github.com/Doomking/auramate/issues/17).
- Visual tone: `docs/requirement.md` §7 (accepted without a separate ticket on the map).

### Media/Meeting, Hyper-Focus, Break Card, Returned, History

- [媒体/会议如何识别，以及如何推迟提醒？](https://github.com/Doomking/auramate/issues/12)
- [深度心流如何进入、表现与记录？](https://github.com/Doomking/auramate/issues/13)
- [Break Card 是否一期必做？](https://github.com/Doomking/auramate/issues/5)
- [归来时如何按上下文接上，而不是冲刷积压？](https://github.com/Doomking/auramate/issues/15)
- [本地历史记录哪些区间、次级页藏多深？](https://github.com/Doomking/auramate/issues/16)

### Defaults (unless a linked ticket says otherwise)

- Focus 25m / Short Break 5m / Long Break 15m after every 4 Focus 时段 (classic defaults; user-facing duration settings may be added later without changing action semantics).
- Snooze delay ≈ 5 minutes; pet re-轻触 during 超时 ≈ every 5 minutes.

## Testing Decisions

- Good tests exercise **external behavior of Rhythm Core only**: given clock + actions + presence observations, assert 时段 transitions, remaining time, 轻触 flags, pet state, 恢复提示 flag, and history interval intents. Do not assert UI widgets, SQL shape, or OS API call sequences in core tests.
- Prefer injecting a controllable clock and presence feed at the core interface over sleeping real time.
- Adapter tests (optional, thinner): Presence adapter maps OS readings into core observations without reading forbidden content; History adapter persists the intervals core emits; smoke-level ambient wiring can wait until after core is green.
- Prior art: none in-repo yet (docs-first). Establish the Rhythm Core test suite as the first automated suite after scaffold.
- Manual / OS checks (human or later tickets): non-activating pet window, tray menu actions, multi-monitor move, macOS sleep vs idle lock fallback — not substitutes for core unit tests.

## Out of Scope

From the map and `docs/requirement.md` phase boundaries:

- Phase 2: pluggable AI, daily review, adaptive rhythm suggestions, chat-first UI
- Phase 3: more pets, persona, extensible pet economy
- Gamification: XP, coins, streaks, check-ins, social, forced Focus
- Default telemetry; capture of screen / key / page contents, mouse trails, detailed app-usage history
- Domain, trademark, store listing names
- Accessibility permission as a phase-1 hard requirement
- Separate corner HUD besides the pet
- Distinguishing meeting vs media as two product states in phase 1
- Private macOS lock APIs

## Further Notes

- Vocabulary: always use `CONTEXT.md` terms (阶段, 时段, 超时, 轻触, 存在, …); avoid synonyms listed under `_Avoid_`.
- After this spec: `/to-tickets` should split tracer-bullet implementation issues with blocking edges, then `/implement` per ticket with a fresh context each time.
- Research assets live on branches `research/presence-signals` and `research/tauri-ambient-windows` until merged; link them from implementation tickets that need OS/Tauri facts.
- Open-source baseline files (`LICENSE`, `CONTRIBUTING.md`, etc.) remain expected for a public repo but are not Rhythm Core behavior; schedule them as separate housekeeping if not already present when scaffolding.
