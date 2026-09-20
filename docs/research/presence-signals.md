# 不采集内容的前提下，系统能提供哪些存在（Presence）信号？

- **Ticket**: [#7](https://github.com/Doomking/auramate/issues/7)
- **范围**: macOS + Windows；一期粗粒度 活动 / 离开 / 锁屏/睡眠 / 媒体/会议 / 「输入仍然频繁」（深度心流前置条件）
- **硬线**（`docs/requirement.md` §4）：不记录屏幕内容、键盘内容、网页内容、鼠标轨迹或详细应用使用历史；Presence 数据默认不上云

本文只陈述官方 API / 系统能力、权限、降级路径，以及隐私边界内**无法可靠区分**的状态。API 名称保持英文。

---

## 结论

活动（Active）与离开（Away）在两平台都有**不读键鼠内容**的公开空闲时间 API：macOS `CGEventSourceSecondsSinceLastEventType`，Windows `GetLastInputInfo`。锁屏/睡眠（Locked/Sleeping）在 Windows 可用会话锁与电源事件分开观察；macOS 睡眠/亮屏有公开通知，但公开 SDK **没有**「屏幕已锁定」键。媒体/会议（Media/Meeting）只有占用类代理（全屏勿扰、麦克风/摄像头是否在跑），**不能**在不采集内容、不记应用历史的前提下把「在开会」和「在看片」分开。「输入仍然频繁」可对空闲时间或事件计数做采样，不必装键鼠 hook。

---

## 隐私边界：可用 vs 明确不做

| 能力 | 是否在边界内 | 依据 |
| --- | --- | --- |
| 距上次输入多少秒 / 上次输入的 tick | 是：只有时间，没有键、没有坐标、没有文本 | macOS `CGEventSourceSecondsSinceLastEventType` 注释写明返回 elapsed time；Windows `GetLastInputInfo` 写明「time of the last input event」，用于 idle detection |
| 睡眠 / 唤醒 / 显示器开关 | 是 | `NSWorkspace` 电源通知；`IORegisterForSystemPower`；Windows `WM_POWERBROADCAST`、`GUID_SESSION_DISPLAY_STATUS` |
| 会话锁定 / 解锁（Windows） | 是 | `WTS_SESSION_LOCK` / `WTS_SESSION_UNLOCK` |
| 麦克风 / 摄像头是否被**某个进程**占用（布尔） | 有条件：只保留布尔，丢弃 PID / 应用名 | `kAudioDevicePropertyDeviceIsRunningSomewhere`、`kCMIODevicePropertyDeviceIsRunningSomewhere`、`AVCaptureDevice.isInUseByAnotherApplication`；Windows `IMFSensorActivityMonitor` 示例把报告收成 `inUse` |
| 系统「现在是否适合弹通知」 | 有条件：可作勿扰代理，不能当会议检测 | Windows `SHQueryUserNotificationState` |
| `CGEventTap` / `NSEvent.addGlobalMonitorForEventsMatchingMask` / `SetWindowsHookEx(WH_KEYBOARD_LL\|WH_MOUSE_LL)` | **否**：回调里是真实按键与指针事件 | 见下文「越界 API」 |
| `CGWindowListCreateImage` / Screen Recording | **否**：屏幕像素 | `CGWindow.h`：`CGWindowListCreateImage` 生成图像；`CGRequestScreenCaptureAccess` 明确是 screen capture |
| 枚举窗口主人 PID/名称、GSMTC 曲目信息、音频会话进程名并落库 | **否**：详细应用使用历史或媒体内容 | `kCGWindowOwnerPID` / `kCGWindowOwnerName` / `kCGWindowName`；GSMTC `TryGetMediaPropertiesAsync` |
| CoreAudio 语音活动检测 | **否**：分析输入音频是否有人声 | `kAudioDevicePropertyVoiceActivityDetectionState` 注释写明 detection works on input audio |

---

## macOS

### 活动 / 离开 / 「输入仍然频繁」

**首选（无辅助功能权限）**：`CGEventSourceSecondsSinceLastEventType(stateID, eventType)`（`API_AVAILABLE(macos(10.4))`）。

头文件原文：返回 elapsed time since the last event；「To get the elapsed time since the previous input event — keyboard, mouse, or tablet — specify `kCGAnyInputEventType`」。

- 状态表用 `kCGEventSourceStateCombinedSessionState`（当前登录会话里所有 event source 的合并态）或 `kCGEventSourceStateHIDSystemState`（HID 硬件事件合并态）。后者更接近「人碰了键鼠」，较少被合成事件污染。
- 公开头文件里**没有** `docs/requirement.md` §6 写的 `CGEventSourceSecondsSinceLastOSXEvent`。IOKit 已废弃的 `NXIdleTime` 注释写：See `CGEventSourceSecondsSinceLastEventType`。
- 同头文件还有 `CGEventSourceCounterForEventType`：自 Window Server 启动以来某类事件的计数；**不计入** key autorepeat；得到的是计数不是按键内容。对计数做短窗口差分，可作「输入仍然频繁」的辅助，而不必读 `characters`。
- 权限：上述查询不要求 Accessibility / Input Monitoring。

**IOKit 备援**：`kIOHIDIdleTimeKey`（`"HIDIdleTime"`）与 `kIOHIDIdleTimeMicrosecondsKey`（`"HIDIdleTimeMicroseconds"`，注释：HID idle time in microseconds）。这是 IORegistry 属性名，不是独立的「空闲秒数」函数；当前公开替代路径仍是 `CGEventSourceSecondsSinceLastEventType`。

**不要用（越界）**：

- `CGEventTapCreate`：tap 回调拿到 Quartz 事件本身；头文件写明 key up/down 需要 assistive device access，否则 mask 会被清掉。这是键内容通道，不是空闲秒数。
- `NSEvent.addGlobalMonitorForEventsMatchingMask:handler:`：handler 收到其他应用的 `NSEvent` 副本；「Key-related events may only be monitored if … trusted for accessibility access」。
- `CGEventSourceKeyState`：按 `CGKeyCode` 查询某键是否按下，属于按键级状态，不是粗粒度存在。

**「输入仍然频繁」在边界内的做法**：周期性读取「距上次任意输入的秒数」（或计数差分）。若连续多个采样都接近 0，即输入仍在发生。不需要坐标序列，也不需要字符。

**活动 vs 离开**：产品自己设空闲阈值（系统 API 不提供「Away」枚举）。`GetLastInputInfo` 的 Windows 文档提醒 tick **不保证单调**（`SendInput` 可自带 tick）；macOS 合成事件同样可能刷新 Combined Session 态，故 HID 态更稳，但仍无法区分「人在输入」与「软件在灌事件」。

### 锁屏 / 睡眠

| 信号 | API | 权限 | 能说明什么 |
| --- | --- | --- | --- |
| 系统即将睡 / 已醒 | `NSWorkspace.willSleepNotification` / `didWakeNotification` | 无特殊 TCC；须用 **`NSWorkspace.shared.notificationCenter`**，用错 center 收不到 | 睡眠，不是锁屏 |
| 显示器睡 / 醒 | `NSWorkspace.screensDidSleepNotification` / `screensDidWakeNotification` | 同上 | 亮屏变化；文档说「Not many apps use this」 |
| 关机 | `NSWorkspace.willPowerOffNotification` | 同上 | 不是睡眠 |
| Fast User Switching 切出 / 切回 | `NSWorkspace.sessionDidResignActiveNotification` / `sessionDidBecomeActiveNotification` | 同上 | **用户会话被切走**，不是锁屏 |
| 更底层睡眠生命周期 | `IORegisterForSystemPower`：`kIOMessageSystemWillSleep`、`kIOMessageSystemHasPoweredOn` 等 | 无特殊 TCC；`kIOMessageSystemWillSleep` **必须** `IOAllowPowerChange`，否则约 30s 后仍会睡 | 与 AppKit 通知同类，粒度更细 |
| 是否在控制台会话 / 是否已 login | `CGSessionCopyCurrentDictionary`：`kCGSessionOnConsoleKey`、`kCGSessionLoginDoneKey` | 无窗口服务器则返回 NULL | 控制台会话，**不是**锁 |

公开 `CGSession.h` 列出的字典键只有 UserID、UserName、ConsoleSet、OnConsole、LoginDone。**没有** ScreenLocked。`notify_post` 公开名只有 `com.apple.coregraphics.GUIConsoleSessionChanged` 与 `GUISessionUserChanged`。

**结论（锁屏）**：在已核对的公开 SDK 头文件与 AppKit 文档里，**没有**受支持的「屏幕已锁定」布尔。业界常用的 Darwin 名（如 `com.apple.screenIsLocked`）**不在**这些公开头文件中，不能当作稳定合约。一期若必须覆盖锁屏，macOS 侧只能：把睡眠 / 显示器睡眠 / 会话切出并进 Locked/Sleeping 桶；真正的 lock 与「人离开但未锁」无法用公开 API 切开。

ScreenSaver.framework 公开 API 是写屏保 UI（`ScreenSaverView`），不是「屏保是否正在跑」的查询接口。

### 媒体/会议

可观察的是**设备占用**，不是「正在开会」或「正在看片」。

| 信号 | API | 权限 | 局限 |
| --- | --- | --- | --- |
| 某音频设备是否在系统**任一进程**里 running | `kAudioDevicePropertyDeviceIsRunningSomewhere`：`UInt32`，1 = running in at least one process | 查询 HAL 属性通常无需 Microphone TCC；**打开** `AVCaptureSession` 采集音频才要授权 | 系统音、音乐、会议、语音消息无法区分；输出设备 running ≠ 会议 |
| 某 CMIO 设备是否在任一进程 running | `kCMIODevicePropertyDeviceIsRunningSomewhere`（同语义） | 同上；不要为了探测去开 capture session | 摄像头占用 ≠ 会议（FaceTime / Photo Booth / 虚拟摄像头皆可） |
| 另一应用是否在用该 capture device | `AVCaptureDevice.isInUseByAnotherApplication`（macCatalyst 14+ / Mac；KVO） | 列出设备与读此布尔，头文件未要求先授权；`authorizationStatusForMediaType:` / `requestAccessForMediaType:` 是**本应用**采集硬件的授权 | 仍是占用，不是会议 |
| 本应用能否用摄像头/麦克风 | `AVAuthorizationStatus` | 未决定时可提示用户 | **不是**占用信号 |

**不要**：为探测而 `requestAccess` 再开 session（会采到内容）；不要用 `kAudioDevicePropertyVoiceActivityDetectionEnable` / `...State`（对输入音频做人声检测）；不要把 `kCGWindowOwnerName` 与窗口标题做成应用时间线。

`CGWindowListCopyWindowInfo` 可给出 `kCGWindowBounds`（屏幕空间矩形）和可选 `kCGWindowName`。用像素图 `CGWindowListCreateImage` 已标 `SCREEN_CAPTURE_OBSOLETE`，且须屏幕录制权限（`CGRequestScreenCaptureAccess`）。即使只根据 bounds 判断「有窗口铺满主屏」，仍无法分辨游戏 / 片 / 会议 / 全屏编辑器；若再记下 PID/应用名，就越过「详细应用使用历史」。

### macOS 降级路径

1. **最低权限**：只轮询 `CGEventSourceSecondsSinceLastEventType(..., kCGAnyInputEventType)` → 活动 / 离开；订阅 `didWake` / `willSleep` / `screensDidSleep` / `screensDidWake` → 并入锁屏/睡眠桶。不申请 Accessibility、Input Monitoring、Screen Recording、Camera、Microphone。
2. **可选占用层**（仍不采内容）：只读 `DeviceIsRunningSomewhere` / `isInUseByAnotherApplication` 的布尔，不存设备名以外的进程身份。失败或拒绝则媒体/会议退化为「无此子状态」，提醒策略按普通活动。
3. **不要**把 Accessibility 当空闲检测的升级路径：它打开的是事件内容，不是更好的秒数。

---

## Windows

### 活动 / 离开 / 「输入仍然频繁」

**首选**：`GetLastInputInfo`（`winuser.h`，Windows 2000+ desktop）。文档：Retrieves the time of the last input event；**useful for input idle detection**；信息只覆盖**调用所在会话**，不是全机所有 session。

`LASTINPUTINFO.dwTime` 是收到上次输入时的 tick，配合 `GetTickCount` 得空闲时长。文档警告：tick **不保证递增**（raw input 与 desktop 线程时差，或 `SendInput` 自带 tick）。

权限：无特殊 capability。

系统级用户是否在提供输入（Win8+，交互会话）：`RegisterPowerSettingNotification` + `GUID_SESSION_USER_PRESENCE`（`3C0F4548-C03F-4C4D-B9F2-237EDE686376`）。`USER_ACTIVITY_PRESENCE`：

- `PowerUserPresent`：The user is providing input to the session（GUID 页对 session 变体的说明）
- `PowerUserInactive`：The user activity timeout has elapsed with no interaction from the user
- `PowerUserNotPresent`：用户不在该会话相关意义上的 present

这是 OS 自己的存在分类，仍无键内容。Session 0 服务应改用 `GUID_GLOBAL_USER_PRESENCE`。桌面伴侣应走 session 变体。

**「输入仍然频繁」**：轮询 `dwTime` 是否持续刷新（或 `PowerUserPresent` 保持）。没有公开的「本会话按键计数」API 可与 macOS `CGEventSourceCounterForEventType` 对等；**不要**为此安装 `WH_KEYBOARD_LL`。

**不要用**：`SetWindowsHookExW` 的 `WH_KEYBOARD_LL` / `WH_MOUSE_LL`。文档写明 low-level keyboard/mouse input events；`LowLevelKeyboardProc` 会拿到虚拟键码。这是键鼠内容/轨迹通道。

### 锁屏 / 睡眠

| 信号 | API | 权限 | 能说明什么 |
| --- | --- | --- | --- |
| 会话锁 / 解锁 | `WTSRegisterSessionNotification` → `WM_WTSSESSION_CHANGE`：`WTS_SESSION_LOCK` (0x7) / `WTS_SESSION_UNLOCK` (0x8) | 对本会话用 `NOTIFY_FOR_THIS_SESSION`；须在窗口销毁前 `WTSUnRegisterSessionNotification` | **明确的锁屏** |
| 进入低功耗 / 恢复 | `WM_POWERBROADCAST`：`PBT_APMSUSPEND`、`PBT_APMRESUMEAUTOMATIC`、`PBT_APMRESUMESUSPEND`（后者表示因用户输入而恢复） | 桌面窗口默认可收；另有 `RegisterSuspendResumeNotification` | 睡眠；文档写明**不能**区分具体低功耗档位 |
| 本会话显示器开/关/变暗 | `RegisterPowerSettingNotification(..., GUID_SESSION_DISPLAY_STATUS, ...)` → `PBT_POWERSETTINGCHANGE`：`PowerMonitorOff/On/Dim` | Win8+ 用户态交互会话 | 关屏 ≠ 锁 ≠ 睡 |
| 盖子 | `GUID_LIDSWITCH_STATE_CHANGE` | 同上 | 合盖，常与睡/关屏伴随 |

`SHQueryUserNotificationState` 的 `QUNS_NOT_PRESENT` 把 **屏保、已锁定、非活动 Fast User Switching** 合成一个值，**不能**单独当锁屏。锁应用 `WTS_SESSION_LOCK`。

`GetSystemPowerStatus` 只报 AC/电池/Battery Saver，与人是否在场无关。

### 媒体/会议

**勿扰 / 全屏代理（不读屏幕像素）**：`SHQueryUserNotificationState`（Vista+）。文档：在弹出类似 `Shell_NotifyIcon` 气球前应先查；仅当 `QUNS_ACCEPTS_NOTIFICATIONS` 时才适合发这类通知。枚举（`QUERY_USER_NOTIFICATION_STATE`）：

| 值 | 官方含义 | 对 AuraMate |
| --- | --- | --- |
| `QUNS_RUNNING_D3D_FULL_SCREEN` | exclusive mode Direct3D 全屏 | 可能是游戏或全屏片；**不是**会议 |
| `QUNS_PRESENTATION_MODE` | 用户打开了 Windows presentation settings | 演示勿扰 |
| `QUNS_BUSY` | 全屏应用 **或** Presentation Settings | 二者合一 |
| `QUNS_NOT_PRESENT` | 屏保 / 锁 / 非活动 FUS | 与锁屏/睡眠重叠，优先用 WTS/电源 API |
| `QUNS_QUIET_TIME` | 新账户首小时或升级后安静时段 | 与存在无关 |
| `QUNS_APP` | Win8+：A Windows Store app is running | 几乎恒真，**不能**当存在信号 |

备注：开始/结束全屏应用**没有**通知；只有 presentation on/off 与 lock/unlock 会 `WM_SETTINGCHANGE`。须轮询。这是「别打扰」信号，不是「正在开会」。

**通信流（会议相邻）**：`IAudioSessionManager2::RegisterDuckNotification`。文档：Windows 7+ ducking；当**默认通信设备**上打开/关闭 communication stream 时系统会发会话事件。这是公开的「可能在通话」代理，仍会把任何通信客户端算进去，且收不到未走通信端点的会议。

**摄像头占用**：`MFCreateSensorActivityMonitor` + `IMFSensorActivityMonitor`。官方示例遍历 `IMFSensorProcessActivity`，用 `GetStreamingState` 得到是否 streaming，再收成布尔 `inUse`。API **能**列出进程；产品若只留布尔、不记进程，才落在隐私硬线内。超时无 report 时示例视为 camera not in use。

**本应用设备权限**（`DeviceAccessInformation.CurrentStatus` / `DeviceAccessStatus`）只说明**本应用**能不能用设备，**不是**系统占用。

**系统媒体会话**：`GlobalSystemMediaTransportControlsSessionManager`（Windows 10 1809+）「playback sessions throughout the system」；单会话有 `GetPlaybackInfo`、`TryGetMediaPropertiesAsync`（曲目等）。播放状态布尔勉强可用；**媒体属性是内容**，不应采集或存储。未接入 SMTC 的播放器不会出现。

`IAudioSessionManager2::GetSessionEnumerator` 可列出会话。若再取 display name / process id 并记录，即应用使用历史。最多把「是否存在 Active 会话」收成布尔。

### Windows 降级路径

1. **最低权限**：`GetLastInputInfo` → 活动/离开；`WTSRegisterSessionNotification` → 锁；`WM_POWERBROADCAST` → 睡。不申请麦克风/摄像头、不安 hook。
2. **勿扰层**：`SHQueryUserNotificationState`。`QUNS_RUNNING_D3D_FULL_SCREEN` / `QUNS_PRESENTATION_MODE` / `QUNS_BUSY` 时推迟视觉轻触。失败则当作 `QUNS_ACCEPTS_NOTIFICATIONS`。
3. **可选占用层**：ducking 通知与/或 sensor activity 布尔。拒绝或 API 不可用则没有媒体/会议子状态。
4. **不要**把低级键鼠 hook 当「更准的空闲检测」。

---

## Tauri 2

官方插件索引（Notifications、global-shortcut、窗口、托盘等）**没有** idle / presence / last-input 插件。空闲、锁、电源、设备占用属于 OS API，应写在 Rust 侧，经 [Calling Rust from the Frontend](https://v2.tauri.app/develop/calling-rust/) 的 IPC 把**已经收成的粗粒度存在枚举**交给 UI，而不是把窗口列表或事件流丢到前端。

系统通知插件只负责**发出**通知；它不能替代 `SHQueryUserNotificationState` 或 `NSWorkspace` 观察。

---

## 领域状态：能区分 / 不能区分

对照 `CONTEXT.md`：

| 状态 | macOS（隐私边界内） | Windows（隐私边界内） |
| --- | --- | --- |
| **活动** | 空闲秒数低于产品阈值 | `GetLastInputInfo` 空闲低，或 `PowerUserPresent` |
| **离开** | 空闲秒数高于阈值，且未进入睡眠桶 | 空闲高或 `PowerUserInactive`，且未锁、未睡 |
| **锁屏/睡眠** | 睡眠/亮屏/会话切出可观测；**锁屏本身无公开 API**，只能与睡眠、关屏粗并 | **锁**（WTS）与**睡**（`PBT_APMSUSPEND`）与**关屏**（display GUID）可分 |
| **归来** | 空闲重新变短，或 `didWake` / `sessionDidBecomeActive` 之后 | `WTS_SESSION_UNLOCK`、`PBT_APMRESUMESUSPEND`、空闲跌回阈值 |
| **媒体/会议** | 仅有「音频/视频设备 somewhere running」；**不能**分会议 vs 片 vs 本地录音 | 勿扰枚举 + 通信 ducking + 摄像头 in-use 布尔；**仍不能**可靠互分；`QUNS_APP` / `QUNS_QUIET_TIME` 无用 |
| **深度心流所需「输入仍然频繁」** | 空闲采样或 event counter 差分 | 空闲 tick 持续刷新 |

**无法在硬线内可靠区分的（两平台）**：

1. 媒体 vs 会议 vs 全屏工作 vs 游戏  
2. 「人在」vs「脚本/SendInput/自动键鼠在动」  
3. 「离开未锁」vs macOS「已锁仍未睡」（缺公开 lock 信号）  
4. 看网页/写文档/刷消息（需要内容或应用历史）  
5. 会议软件开着但人已走开（占用仍为真，空闲已高：产品应让空闲/锁优先于占用）

**归来**不是 OS 枚举，是产品把「从离开或锁屏/睡眠回到活动」接出来的状态。

---

## 一期可用的最小集合（事实，不是实现承诺）

在零额外 TCC / 零 hook 下已经够支撑「到点先看人在不在」：

- 活动 / 离开：上次输入距今  
- 锁屏/睡眠：Windows 用 WTS + 电源；macOS 用 sleep/wake（锁降级进同一桶）  
- 归来：上述反向边沿  
- 媒体/会议：视为**可选、可失败**的子状态；Windows 可先用 `SHQueryUserNotificationState` 的 busy/fullscreen/presentation 做「推迟轻触」，不要号称识别了会议  
- 输入仍然频繁：对空闲时间采样，不读键

---

## 来源

### Apple（文档站点）

- [NSWorkspace.didWakeNotification](https://developer.apple.com/documentation/appkit/nsworkspace/didwakenotification)
- [NSWorkspace.willSleepNotification](https://developer.apple.com/documentation/appkit/nsworkspace/willsleepnotification)
- [NSWorkspace.screensDidSleepNotification](https://developer.apple.com/documentation/appkit/nsworkspace/screensdidsleepnotification)
- [NSWorkspace.screensDidWakeNotification](https://developer.apple.com/documentation/appkit/nsworkspace/screensdidwakenotification)
- [NSWorkspace.sessionDidBecomeActiveNotification](https://developer.apple.com/documentation/appkit/nsworkspace/sessiondidbecomeactivenotification)
- [NSWorkspace.sessionDidResignActiveNotification](https://developer.apple.com/documentation/appkit/nsworkspace/sessiondidresignactivenotification)
- [CGEventSourceStateID](https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid)
- [AVCaptureDevice.isInUseByAnotherApplication](https://developer.apple.com/documentation/avfoundation/avcapturedevice/isinusebyanotherapplication)
- [kAudioDevicePropertyDeviceIsRunningSomewhere](https://developer.apple.com/documentation/coreaudio/kaudiodevicepropertydeviceisrunningsomewhere)
- [AVCaptureDevice.authorizationStatus(for:)](https://developer.apple.com/documentation/avfoundation/avcapturedevice/authorizationstatus(for:))

### Apple（MacOSX SDK 头文件，Command Line Tools `MacOSX.sdk`）

- `CoreGraphics/CGEventSource.h` — `CGEventSourceSecondsSinceLastEventType`、`CGEventSourceCounterForEventType`
- `CoreGraphics/CGEventTypes.h` — `kCGEventSourceStateCombinedSessionState`、`kCGEventSourceStateHIDSystemState`、`kCGAnyInputEventType`
- `CoreGraphics/CGEvent.h` — `CGEventTapCreate` 与 assistive access
- `CoreGraphics/CGSession.h` — `CGSessionCopyCurrentDictionary` 公开键
- `CoreGraphics/CGWindow.h` — 窗口字典键、`CGWindowListCreateImage`、`CGRequestScreenCaptureAccess`
- `AppKit/NSWorkspace.h` — 电源 / 会话通知
- `AppKit/NSEvent.h` — `addGlobalMonitorForEventsMatchingMask:handler:`
- `IOKit/hidsystem/event_status_driver.h` — `NXIdleTime` → `CGEventSourceSecondsSinceLastEventType`
- `IOKit/hidsystem/IOHIDParameter.h` — `kIOHIDIdleTimeKey`
- `IOKit/hid/IOHIDProperties.h` — `kIOHIDIdleTimeMicrosecondsKey`
- `IOKit/pwr_mgt/IOPMLib.h` — `IORegisterForSystemPower`
- `CoreAudio/AudioHardware.h` — `kAudioDevicePropertyDeviceIsRunningSomewhere`、Voice Activity Detection
- `CoreMediaIO/CMIOHardwareDevice.h` — `kCMIODevicePropertyDeviceIsRunningSomewhere`
- `AVFoundation/AVCaptureDevice.h` — `isInUseByAnotherApplication`、`authorizationStatusForMediaType:`
- `HIServices/AXUIElement.h` — `AXIsProcessTrustedWithOptions`

### Microsoft Learn

- [GetLastInputInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo)
- [LASTINPUTINFO](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-lastinputinfo)
- [SHQueryUserNotificationState](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shqueryusernotificationstate)
- [QUERY_USER_NOTIFICATION_STATE](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ne-shellapi-query_user_notification_state)
- [WTSRegisterSessionNotification](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsregistersessionnotification)
- [WM_WTSSESSION_CHANGE](https://learn.microsoft.com/en-us/windows/win32/termserv/wm-wtssession-change)
- [WM_POWERBROADCAST](https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast)
- [RegisterPowerSettingNotification](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerpowersettingnotification)
- [Power Setting GUIDs](https://learn.microsoft.com/en-us/windows/win32/power/power-setting-guids)（`GUID_SESSION_USER_PRESENCE`、`GUID_SESSION_DISPLAY_STATUS`、`GUID_CONSOLE_DISPLAY_STATE`、`GUID_LIDSWITCH_STATE_CHANGE`）
- [USER_ACTIVITY_PRESENCE](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-user_activity_presence)
- [IAudioSessionManager2](https://learn.microsoft.com/en-us/windows/win32/api/audiopolicy/nn-audiopolicy-iaudiosessionmanager2)
- [IMFSensorActivityMonitor](https://learn.microsoft.com/en-us/windows/win32/api/mfidl/nn-mfidl-imfsensoractivitymonitor)
- [GlobalSystemMediaTransportControlsSession](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession)
- [GlobalSystemMediaTransportControlsSessionManager](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager)
- [DeviceAccessInformation](https://learn.microsoft.com/en-us/uwp/api/windows.devices.enumeration.deviceaccessinformation)
- [SetWindowsHookExW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw)
- [SetThreadExecutionState](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setthreadexecutionstate)

### Tauri

- [Plugins / Features](https://v2.tauri.app/plugin/)
- [Calling Rust from the Frontend](https://v2.tauri.app/develop/calling-rust/)
