# Tauri 2：非抢焦点窗、托盘、透明置顶、多屏窗口的能力边界

**Ticket:** [#8](https://github.com/Doomking/auramate/issues/8)
**Scope:** macOS 与 Windows 上，官方配置 / JS / Rust API 能做什么，以及它们映射到的 OS API。只记事实。
**Snapshot:** 2026-09-20。公开文档以 [v2.tauri.app](https://v2.tauri.app/) 为准；实现细节来自 `tauri-apps/tauri` `dev`、`tauri-apps/tao` `dev`、`tauri-apps/tray-icon` `dev`。

对本产品的含义用 `CONTEXT.md` 的词：轻触、宠物、提醒。不另造同义词。

## 一句话结论

默认窗口会抢焦点。非抢焦点没有单独的「ambient window」类型，而是 `focus: false`（创建时）+ `focusable: false`（Windows 设 `WS_EX_NOACTIVATE`，macOS 覆盖 `canBecomeKeyWindow` / `canBecomeMainWindow`）。托盘菜单可以挂暂停 / 跳过 / 延后这类自定义项。透明置顶可配，但 macOS 透明背景要 `macOSPrivateApi`（上不了 App Store）。Windows 不支持 `visibleOnAllWorkspaces`。没有「把窗口放到第 N 块屏」的一等 API，只能列出 `Monitor`、把物理坐标换成逻辑坐标，再 `setPosition`。

---

## 1. 默认会不会抢焦点？

会。默认值就是「可见、可聚焦、创建时聚焦」。

| 字段 | 默认 | 文档含义 |
| --- | --- | --- |
| `visible` | `true` | 创建后立刻可见 |
| `focusable` | `true` | 窗口可以成为焦点 |
| `focus` | `true` | 窗口**最初会被聚焦** |

来源：[WindowConfig](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)（`focus` / `focusable` / `visible` 的 rustdoc 与 `Default`）；JS [WindowOptions](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)；底层 [tao `WindowAttributes`](https://github.com/tauri-apps/tao/blob/dev/src/window.rs) 同样 `focused: true`、`focusable: true`、`visible: true`。公开入口：[WindowConfig](https://v2.tauri.app/reference/config/#windowconfig)。

### 创建时：`focus` 决定第一次出现会不会变成 key / foreground

**macOS（tao 创建路径）**

- `visible && focused` → `NSWindow.makeKeyAndOrderFront`。Apple：把窗口带到其所在 level 的最前，**并使其成为 key window**。[makeKeyAndOrderFront(_:)](https://developer.apple.com/documentation/appkit/nswindow/makekeyandorderfront(_:))
- `visible && !focused` → `NSWindow.orderFront`。Apple：带到最前，**不改变 key window 或 main window**。[orderFront(_:)](https://developer.apple.com/documentation/appkit/nswindow/orderfront(_:))

来源：[tao `platform_impl/macos/window.rs`](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/macos/window.rs) 创建末尾的 `if focused { makeKeyAndOrderFront } else { orderFront }`。

**Windows（tao 创建 / 第一次 `ShowWindow`）**

- `focused == false` 时设 `WindowFlags::MARKER_DONT_FOCUS`，第一次显示走 `SW_SHOWNOACTIVATE`。MSDN：显示窗口但**不激活**。[ShowWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow)
- 否则走 `SW_SHOW`。MSDN：激活窗口并以当前尺寸显示。

来源：[tao `window_state.rs`](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/windows/window_state.rs)（`MARKER_DONT_FOCUS` → `SW_SHOWNOACTIVATE`，然后清掉该标记）。

### 之后再 `show()`：macOS 会走「成为 key」那条路

`Window.show()` 的公开文档只写「把可见性设为 true」，没有写会不会聚焦。[window.ts `show()`](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

实现上：

- **macOS `set_visible(true)`** 调用 `make_key_and_order_front_sync` → `makeKeyAndOrderFront`。[tao macos window.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/macos/window.rs)、[util/async.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/macos/util/async.rs)
- 因此：创建时用 `focus: false` 可以 `orderFront` 不改 key；**后来再 `Window.show()` 仍会走 makeKeyAndOrderFront**。公开 Window API 没有对应的 `orderFront` / `showWithoutActivating`。
- **Windows**：`MARKER_DONT_FOCUS` 只用于第一次显示。之后的 `ShowWindow` 走 `SW_SHOW`（会激活），除非窗口带着下面的 `focusable: false` / `WS_EX_NOACTIVATE`。

官方托盘教程在左键处理里主动调用 `unminimize` + `show` + `set_focus`，把主窗拉到前台并聚焦；这是示例，不是默认托盘行为。[System Tray](https://v2.tauri.app/learn/system-tray/)

### 显式抢焦点 / 注意力 API

- `Window.setFocus()`：**Bring the window to front and focus.** [window.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)
- `Window.requestUserAttention(Critical | Informational)`：应用已在前台时无效果。macOS Critical 会一直弹 Dock 图标直到聚焦，Informational 弹一次；Windows Critical 闪窗口+任务栏直到聚焦，Informational 闪任务栏直到聚焦。[UserAttentionType](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)
- `@tauri-apps/api/app` 的 `show()`（macOS）：显示应用，**不会自动聚焦任何一个窗口**。这与 `Window.show()` 不是同一件事。[app.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/app.ts)

---

## 2. 非抢焦点窗口：官方能力就是 `focusable`

没有 `NSPanel`、没有 `NSWindowStyleMaskNonactivatingPanel`、没有 Windows `WS_EX_NOACTIVATE` 的独立配置项。公开开关是 `focusable`（配置 / `WindowOptions` / `setFocusable`）。

### `focusable: false` 映射到 OS

**macOS**

tao 把 `NSWindow` 子类化成 `TaoWindow`，用 ivar `focusable` 覆盖：

- `canBecomeKeyWindow`
- `canBecomeMainWindow`

来源：[tao macos window.rs `WINDOW_CLASS`](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/macos/window.rs)

Apple：`canBecomeKey` 为 false 时，**试图把该窗口变成 key window 的操作会被放弃**。[canBecomeKey](https://developer.apple.com/documentation/appkit/nswindow/canbecomekey)

文档写明的限制：**如果窗口已经是 focused，再 `setFocusable(false)` 无法把它从焦点拿掉**；建议再调 `setFocus`，但那会把窗口移到 z-order 底部。[window.ts `setFocusable`](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)；[tao `Window::set_focusable`](https://github.com/tauri-apps/tao/blob/dev/src/window.rs)

**Windows**

`!FOCUSABLE` → 扩展样式 `WS_EX_NOACTIVATE`。[tao window_state.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/windows/window_state.rs)

MSDN：带此样式的顶层窗口，用户点击时**不会成为 foreground**；当前前景窗口最小化或关闭时，系统也不会把它带到前台。[Extended Window Styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)

### 应用级：macOS 激活策略与 Dock

`AppHandle::set_activation_policy` / `App::set_activation_policy`，**默认 `NSApplicationActivationPolicyRegular`**。[tauri `app.rs`](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/app.rs)；枚举 [tauri-runtime `ActivationPolicy`](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-runtime/src/lib.rs)

对应 Apple [`NSApplication.ActivationPolicy`](https://developer.apple.com/documentation/appkit/nsapplication/activationpolicy-swift.enum)：

| Tauri | Apple | Apple 含义 |
| --- | --- | --- |
| `Regular` | `.regular` | 普通应用，出现在 Dock，可以有 UI。捆绑应用默认。 |
| `Accessory` | `.accessory` | **不出现在 Dock、没有菜单栏**；仍可通过编程或点窗口激活。对应 `Info.plist` 的 `LSUIElement = 1`。 |
| `Prohibited` | `.prohibited` | 不出现在 Dock，**不可创建窗口、不可被激活**。对应 `LSBackgroundOnly = 1`。 |

`setDockVisibility(false)`（JS `@tauri-apps/api/app`，since 2.5.0）单独控制 Dock 可见性。[app.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/app.ts)

`skipTaskbar`：**配置注释写「Windows 和 Linux 上隐藏任务栏图标」**；JS `setSkipTaskbar` 写 **macOS: Unsupported**。runtime 在 macOS / iOS / Android 上 `skip_taskbar` 是空操作。[WindowConfig](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)；[window.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)；[tauri-runtime-wry](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-runtime-wry/src/lib.rs)

Windows 实现：`ITaskbarList::DeleteTab` / `AddTab`。[tao windows window.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/windows/window.rs)。MSDN：`DeleteTab` 从任务栏删除一项。[ITaskbarList::DeleteTab](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itaskbarlist-deletetab)

---

## 3. 系统托盘 / 菜单栏

Tauri 2 把托盘放在 core（feature `tray-icon`），不是单独 plugin。[System Tray](https://v2.tauri.app/learn/system-tray/)；`AppConfig::all_features` 含 `"tray-icon"`。[config.rs](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)

配置：`app.trayIcon` → [`TrayIconConfig`](https://v2.tauri.app/reference/config/#trayiconconfig)（`id`、`iconPath`、`iconAsTemplate`、`showMenuOnLeftClick`、`title`、`tooltip`）。运行时：JS [`TrayIcon`](https://v2.tauri.app/reference/javascript/api/namespacetray/) / Rust [`TrayIconBuilder`](https://docs.rs/tauri/latest/tauri/tray/struct.TrayIconBuilder.html)。

### OS 落点

- **macOS**：`NSStatusBar.systemStatusBar().statusItemWithLength` → [`NSStatusItem`](https://developer.apple.com/documentation/appkit/nsstatusitem)（「显示在系统菜单栏上的单个元素」）。[tray-icon macos](https://github.com/tauri-apps/tray-icon/blob/dev/src/platform_impl/macos/mod.rs)
- **Windows**：`Shell_NotifyIconW(NIM_ADD / NIM_MODIFY / NIM_DELETE)` + `NOTIFYICONDATAW`。[tray-icon windows](https://github.com/tauri-apps/tray-icon/blob/dev/src/platform_impl/windows/mod.rs)；[Shell_NotifyIconW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw)

tray-icon 文档（macOS）：必须在**主线程、事件循环已经在跑**之后创建托盘，否则可能影响全屏应用。[tray-icon lib.rs](https://github.com/tauri-apps/tray-icon/blob/dev/src/lib.rs)

### 托盘菜单能不能承载暂停 / 跳过 / 延后？

能。菜单是原生 `Menu` / `MenuItem` / `CheckMenuItem` / `Submenu` / `IconMenuItem`，带 `id`、`text`、`enabled`、`action` / `on_menu_event`。官方教程用 `id: 'quit'` 演示同一套 API；窗口菜单文档还覆盖 check、separator、禁用、动态 `setText` / `setChecked` / accelerator。[System Tray — Add a Menu](https://v2.tauri.app/learn/system-tray/)；[Window Menu](https://v2.tauri.app/learn/window-menu/)（「Native application menus can be attached to both to a window or system tray.」）

这覆盖暂停、跳过、延后（以及 +5 / +10）作为**自定义菜单项**。官方没有预定义「Snooze」菜单项；`PredefinedMenuItem` 是 Copy / Paste / Undo 这类系统项。[Window Menu — predefined](https://v2.tauri.app/learn/window-menu/)

默认：菜单在**左键和右键**都会弹出；`showMenuOnLeftClick` / `show_menu_on_left_click` 默认 `true`。Linux：该选项 Unsupported。[System Tray note](https://v2.tauri.app/learn/system-tray/)；[TrayIconOptions](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/tray.ts)

### 平台差异（官方写明的）

| 能力 | macOS | Windows |
| --- | --- | --- |
| `tooltip` | 有 | 有 |
| `title`（图标旁文字） | 有 | **Unsupported** |
| `iconAsTemplate` | 有（[NSImage.isTemplate](https://developer.apple.com/documentation/appkit/nsimage/istemplate)） | 无此选项 |
| 托盘鼠标事件（Click / DoubleClick / Enter / Move / Leave） | 有 | 有 |
| `showMenuOnLeftClick` | 有 | 有 |

来源：[tray.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/tray.ts)；[TrayIconConfig](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)。Linux：托盘事件不发射（仍可右键菜单）——本票范围外。

图标、tooltip、title、可见性、菜单都可以事后改（`setIcon` / `setTooltip` / `setTitle` / `setVisible` / `setMenu`），所以托盘可以随阶段变化，而不必打开窗口。

---

## 4. 透明 + 置顶

没有「宠物浮层」一等类型。组合公开开关：

| 开关 | 作用 |
| --- | --- |
| `decorations: false` | 无边框 / 无标题栏。[Window Customization](https://v2.tauri.app/learn/window-customization/) |
| `transparent: true` | 窗口透明 |
| `alwaysOnTop: true` | 保持在其他窗口之上 |
| `shadow` | 阴影（有平台限制） |
| `setIgnoreCursorEvents(true)` | 点击穿透 |
| `windowEffects` | 毛玻璃等；**要求窗口已透明** |

### 透明

**macOS：** `transparent` **需要 `app.macOSPrivateApi` / Cargo feature `macos-private-api`**。文档警告：用 private API **无法上 App Store**。[WindowConfig.transparent](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)；[WindowOptions.transparent](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

未开该 feature 时，debug build 会 eprintln 同一句话，且 `transparent()` builder 方法在 macOS 上根本不编译进去。[tauri-runtime-wry](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-runtime-wry/src/lib.rs)

这与 `titleBarStyle: Transparent | Overlay`（只改标题栏外观）不是同一件事。后者不要求 private API。[Window Customization — macOS transparent titlebar](https://v2.tauri.app/learn/window-customization/)

**Windows：** 可用，无 private API 开关。`noRedirectionBitmap` 设 `WS_EX_NOREDIRECTIONBITMAP`，减轻透明窗创建时白闪。[WindowConfig](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)；MSDN：窗口不渲染到 redirection surface。[Extended Window Styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)

tao 在 `transparent && !no_redirection_bitmap` 时调用 `DwmEnableBlurBehindWindow` 配空 region，做成全透明。[tao windows window.rs `on_create`](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/windows/window.rs)

`Window.setBackgroundColor`：**Windows 忽略 alpha**。[window.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)；[tao `background_color`](https://github.com/tauri-apps/tao/blob/dev/src/window.rs)

### 置顶

`alwaysOnTop` / `setAlwaysOnTop(true)`：

- **macOS：** `NSWindow.setLevel(NSFloatingWindowLevel)`。[tao macos window.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/macos/window.rs)。Apple：`floating` level「Useful for floating palettes.」[NSWindow.Level.floating](https://developer.apple.com/documentation/appkit/nswindow/level-swift.struct/floating)
- **Windows：** `WS_EX_TOPMOST`；运行时 `SetWindowPos(..., HWND_TOPMOST, ...)`。[tao window_state.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/windows/window_state.rs)。MSDN：`HWND_TOPMOST` 把窗口放在所有非 topmost 窗口之上，**停用后仍保持 topmost**。[SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos)

`alwaysOnBottom` 存在（macOS `BelowNormalWindowLevel`；Windows 对应非 topmost / bottom 路径）。

### 点击穿透与阴影

`setIgnoreCursorEvents(true)`：Windows 加 `WS_EX_TRANSPARENT | WS_EX_LAYERED`。[tao window_state.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/windows/window_state.rs)

`shadow`：

- Windows：`false` 对**有装饰**窗口无效（阴影一直在）；`true` 让无装饰窗口有 1px 白边，Windows 11 上还有圆角。
- Linux：Unsupported。

[WindowConfig.shadow](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)；[window.ts `setShadow`](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

`windowEffects` 要求透明；Windows 若同时用 decorations / shadows，文档指向 tao issue 里的 workaround。[WindowOptions.windowEffects](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

### 自定义标题栏（主窗，不是浮层）

`decorations: false` + `data-tauri-drag-region` / `startDragging`。macOS 自定义标题栏会丢掉系统提供的移动/对齐窗口能力。[Window Customization](https://v2.tauri.app/learn/window-customization/)

JS 窗口命令默认被 ACL 拦住，需要 capability（如 `core:window:allow-start-dragging`）。同上。

---

## 5. 把窗口放到指定显示器

没有 `moveToMonitor(index)`。公开组合是：

1. `availableMonitors()` / `primaryMonitor()` / `currentMonitor()` / `monitorFromPoint(x, y)`
2. 读 `Monitor.position` / `size` / `workArea` / `scaleFactor`
3. `setPosition` 或创建时的 `x` / `y`

[window.ts Monitor + helpers](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)；公开文档 [namespacewindow](https://v2.tauri.app/reference/javascript/api/namespacewindow/)

### 已知限制（文档写明的）

**逻辑像素 vs 物理像素。** `Monitor.position` / `size` / `workArea` 是**物理像素**。创建窗口的 `x` / `y` / `width` / `height` 是**逻辑像素**。官方示例：先 `monitor.position.toLogical(monitor.scaleFactor)` 再传入 `WebviewWindow`。[Monitor JSDoc](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

**`preventOverflow` 只在创建时检查。** 可设 `true` 或额外 margin，把初始尺寸限制在工作区（显示器 − 任务栏/Dock − margin）。**之后的 resize 仍可溢出。** iOS / Android：Unsupported。[WindowOptions.preventOverflow](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

**`visibleOnAllWorkspaces`：Windows Unsupported。** macOS 实现为 `NSWindowCollectionBehavior::CanJoinAllSpaces`。[WindowConfig](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)；[tao macos `set_visible_on_all_workspaces`](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/macos/window.rs)。Apple：`canJoinAllSpaces` — 「The window can appear in all spaces. The menu bar behaves this way.」[canJoinAllSpaces](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/canjoinallspaces)

**`setFullscreenOnMonitor(physicalPosition)`**（since 2.12.0）：在包含该物理点的显示器上全屏；没有显示器包含该点则什么都不做。产品不变式是不抢全屏，这条只说明 API 存在。[window.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

**macOS `setSimpleFullscreen`：** Lion 之前那种不占用新 Space 的全屏；窗口已经在原生全屏里则失败。其他平台等同 `setFullscreen`。同上。

**`cursorPosition()` 原点因平台而异：** 桌面左上角不一定等于「屏幕」左上角。多屏时，Windows / macOS 是**主屏**左上；X11 是**最左屏**左上。坐标可以为负。[window.ts `cursorPosition`](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)

**`parent` 绑定的 z-order / 生命周期：**

- Windows：父窗变成 owner。MSDN owned windows：owned 窗口永远在 owner 之上；owner 销毁则 owned 销毁；owner 最小化则 owned 隐藏。[Window Features](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows)
- macOS：`addChildWindow`。[addChildWindow(_:ordered:)](https://developer.apple.com/documentation/appkit/nswindow/addchildwindow(_:ordered:))

轻触 / 宠物如果做成独立窗口，用 `parent` 会绑到主窗的最小化与 z-order。

---

## 6. 对轻触 / 宠物 / 提醒的事实对照

| 产品需要 | Tauri 2 官方能力 | 文档里的硬限制 |
| --- | --- | --- |
| 提醒先改环境（托盘），不抢焦点 | 托盘图标 + 菜单 + tooltip / macOS title；改图标不必 `setFocus` | 官方左键示例调用了 `set_focus()`，那是教程不是默认 |
| 暂停 / 跳过 / 延后放在托盘 | 自定义 `MenuItem` + `id` + click handler | 原生菜单，不是自定义 HTML 面板 |
| 角落 / 宠物浮层，不抢焦点 | `focus: false` 创建；`focusable: false` 阻止成为 key/foreground；`alwaysOnTop`；`decorations: false` | 没有 NSPanel。macOS 之后 `show()` 走 `makeKeyAndOrderFront`（`canBecomeKey` 为 false 时，Apple 说成为 key 会被放弃）。macOS 已聚焦时 `setFocusable(false)` 卸不掉焦点 |
| 透明宠物 | `transparent` + 可选 `windowEffects` / `ignoreCursorEvents` | **macOS 要 `macOSPrivateApi`，不能上 App Store。** Windows 忽略背景 alpha；无装饰 + shadow 有 1px 白边 |
| 浮层放到某块屏 | `availableMonitors` + 物理→逻辑 + `setPosition` | 无「第 N 屏」API。Windows 不能 `visibleOnAllWorkspaces`。`preventOverflow` 只管创建 |
| 不进任务栏 / Dock | Windows：`skipTaskbar`。macOS：应用级 `ActivationPolicy::Accessory` 或 `setDockVisibility(false)`，不是 per-window `skipTaskbar` | `skipTaskbar` 在 macOS 是 no-op |

---

## 7. 官方 API 没有的东西

下列在 Tauri 2 公开 Window / Tray 表面里**找不到**（查了 WindowConfig、WindowOptions、tao WindowAttributes、TrayIconOptions）：

- `NSPanel` / `NSWindowStyleMaskNonactivatingPanel` / 独立的「non-activating panel」类型
- 运行时 `orderFront` / `SW_SHOWNOACTIVATE` / `showWithoutActivating`（只有创建时 `focus: false`）
- Windows 的 `visibleOnAllWorkspaces`
- macOS 的 per-window `skipTaskbar`
- `moveToMonitor(index)` / `setMonitor(id)`
- 托盘菜单里的 HTML / Webview（只有原生菜单项）

前端调用窗口 / 托盘命令还要在 capability 里放行（`core:window:*`、`plugin:tray|*`）。Rust 侧 `TrayIconBuilder` / `WebviewWindowBuilder` 不走 JS ACL。[Window Customization — Permissions](https://v2.tauri.app/learn/window-customization/)

---

## 主要来源

1. [Tauri 2 WindowConfig](https://v2.tauri.app/reference/config/#windowconfig) / [源码 config.rs](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs)
2. [JS window API](https://v2.tauri.app/reference/javascript/api/namespacewindow/) / [window.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/window.ts)
3. [JS tray API](https://v2.tauri.app/reference/javascript/api/namespacetray/) / [tray.ts](https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/tray.ts)
4. [System Tray](https://v2.tauri.app/learn/system-tray/) · [Window Customization](https://v2.tauri.app/learn/window-customization/) · [Window Menu](https://v2.tauri.app/learn/window-menu/)
5. [tao window.rs](https://github.com/tauri-apps/tao/blob/dev/src/window.rs) · [macos/window.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/macos/window.rs) · [windows/window_state.rs](https://github.com/tauri-apps/tao/blob/dev/src/platform_impl/windows/window_state.rs)
6. [tauri-runtime-wry](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-runtime-wry/src/lib.rs) · [tauri app.rs ActivationPolicy](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/app.rs)
7. [tray-icon macos](https://github.com/tauri-apps/tray-icon/blob/dev/src/platform_impl/macos/mod.rs) · [windows](https://github.com/tauri-apps/tray-icon/blob/dev/src/platform_impl/windows/mod.rs)
8. Apple: [makeKeyAndOrderFront](https://developer.apple.com/documentation/appkit/nswindow/makekeyandorderfront(_:)) · [orderFront](https://developer.apple.com/documentation/appkit/nswindow/orderfront(_:)) · [canBecomeKey](https://developer.apple.com/documentation/appkit/nswindow/canbecomekey) · [ActivationPolicy](https://developer.apple.com/documentation/appkit/nsapplication/activationpolicy-swift.enum) · [NSStatusItem](https://developer.apple.com/documentation/appkit/nsstatusitem) · [canJoinAllSpaces](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/canjoinallspaces) · [floating level](https://developer.apple.com/documentation/appkit/nswindow/level-swift.struct/floating)
9. Microsoft: [Extended Window Styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles) · [ShowWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow) · [SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos) · [ITaskbarList::DeleteTab](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itaskbarlist-deletetab) · [Owned Windows](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows) · [Shell_NotifyIconW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shell_notifyiconw)
