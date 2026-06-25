# Magnifier Feature Design

**Date:** 2026-06-25
**Status:** Draft
**Scope:** Alt+3 放大镜功能 — 按住显示, 滚轮调倍率, 松开消失

## Overview

放大镜功能: 用户按住修饰键+3 时, 在鼠标位置显示圆形放大镜, 放大底层屏幕内容. 滚轮调整倍率. 松开修饰键关闭. 独立小窗口实现, 与现有 overlay 正交.

## Interaction Model

### Trigger

- **开启**: 修饰键按住 + 按 3 (DigitPressed(3)) → toggle magnifier_active
- **调整**: 修饰键按住期间, 滚轮调倍率
- **关闭**: 松开修饰键 → magnifier_active = false (zoom_level 保留)
- **Escape**: 清除所有状态, 包括放大镜

### Modifier Flow

```
Idle → [modifier down] → Armed → [3 pressed] → magnifier_active = true
                                                ↓
                              [scroll up/down] → zoom_level ± 0.5
                                                ↓
                              [modifier up] → magnifier_active = false → Idle
```

### Independence

magnifier_active 与 pinned_active / spotlight_active 独立. 可同时使用:
- 画矩形时放大镜跟随鼠标
- Spotlight + 放大镜同时生效

### Zoom

- 默认: 2.0x
- 范围: [1.5, 8.0]
- 步长: 0.5 (每格滚轮)
- modifier 松开后保留倍率, 下次开启时沿用

## Architecture

### File Changes

| File | Change |
|------|--------|
| `src/magnifier.rs` | **NEW** — MagnifierWindow, 屏幕捕获, 放大渲染 |
| `src/state.rs` | AppState 加 magnifier_active + zoom_level, process_event 加分支 |
| `src/hook.rs` | decide_keyboard 加 DigitPressed(3), decide_mouse 加 WM_MOUSEWHEEL |
| `src/overlay.rs` | App 持有 MagnifierWindow, render 时调用 magnifier |
| `src/main.rs` | mod magnifier |
| `src/config.rs` | 可选配置项 (不在此迭代实现) |

### State Machine (state.rs)

```rust
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<PinnedRect>,
    pub pinned_active: bool,
    pub spotlight_active: bool,
    pub magnifier_active: bool,  // NEW
    pub zoom_level: f64,         // NEW, default 2.0
}
```

新增/修改 process_event 分支:

| State | Event | Effect |
|-------|-------|--------|
| Armed \| Drawing | DigitPressed(3) | toggle magnifier_active (新增 arm) |
| magnifier_active | ScrollUp | zoom_level = (zoom_level + 0.5).min(8.0) (新增 arm) |
| magnifier_active | ScrollDown | zoom_level = (zoom_level - 0.5).max(1.5) (新增 arm) |
| * | ModifierChanged { false } | **扩展现有 arm**: magnifier_active = false, zoom_level unchanged |
| * | EscapePressed | **扩展现有 arm**: magnifier_active = false |

**注意**: `DigitPressed(3)` 复用现有 `InputEvent::DigitPressed(u8)` 变体, 无需新增枚举成员.

`process_event` 的返回值解构必须扩展:
```rust
// 现有: (drawing, pinned_active, spotlight_active, pinned_rects)
// 改为: (drawing, pinned_active, spotlight_active, magnifier_active, zoom_level, pinned_rects)
```

现有 `ModifierChanged { pressed: false }` arm (state.rs:116, 120) 和 `EscapePressed` arm (state.rs:83) 需要额外设置 `magnifier_active = false`.

### Input Hook (hook.rs)

#### decide_keyboard

```rust
// 新增: 在已有的 if is_key_down { ... } 块内, vk_code 0x33 ('3') → DigitPressed(3)
// 不能放在 is_key_down 块外, 否则 key-up 也会触发 toggle
if modifier_held && vk_code == 0x33 {
    return Some(InputEvent::DigitPressed(3));
}
```

#### decide_mouse

```rust
// 新增: WM_MOUSEWHEEL 必须在 drag_in_progress early-return 之前处理
// 否则 Drawing 状态下滚轮事件会被 (None, false) 丢弃
WM_MOUSEWHEEL => {
    if modifier_held {
        let delta = /* GET_WHEEL_DELTA_WPARAM */;
        if delta > 0 { (Some(InputEvent::ScrollUp), true) }
        else { (Some(InputEvent::ScrollDown), true) }
    } else {
        (None, false)
    }
}
```

`should_suppress = true` — modifier 按住时拦截滚轮, 防止其他应用响应.

**注意**: 仅当 `magnifier_active` 时才 suppress scroll. modifier 按住但未按 3 时, 滚轮应正常传递给其他应用. `decide_mouse` 需要访问 `magnifier_active` 状态 (通过参数传入或读取全局状态).

### InputEvent 扩展 (state.rs)

```rust
pub enum InputEvent {
    // ... existing variants (包括 DigitPressed(u8)) ...
    ScrollUp,    // NEW
    ScrollDown,  // NEW
}
```

`DigitPressed(3)` 复用现有 `DigitPressed(u8)` 变体, 无需新增枚举成员.

### Magnifier Window (magnifier.rs)

#### Window Properties

- Style: `WS_POPUP`
- ExStyle: `WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT`
- Size: diameter × diameter (diameter ≈ 350)
- Position: cursor_pos - (diameter/2, diameter/2)
- 不在任务栏显示 (WS_EX_TOOLWINDOW)
- 不激活 (不抢焦点)
- **Owner**: overlay 窗口 (通过 `SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, overlay_hwnd)` 设置), 保证 Z-order 在 overlay 之上

#### Render Cycle (每帧)

```
1. ShowWindow(hwnd, SW_HIDE)        // 隐藏, 避免 BitBlt 捕获到自己
2. BitBlt(screen_dc → mem_dc)       // 捕获鼠标周围区域 (直径/zoom × 直径/zoom)
3. StretchBlt(mem_dc → dib_dc)      // 放大到 直径 × 直径
4. 圆形裁剪:
   BeginPath → Ellipse → EndPath → SelectClipPath
5. 彩虹描边边框 (见下方说明)
6. 倍率文字 ("2.0x") 在底部居中
7. UpdateLayeredWindow              // 先提交新内容到窗口
8. ShowWindow(hwnd, SW_SHOW)        // 再显示 — 此时窗口已携带新内容, 无闪烁
```

步骤 7 在 8 之前: `UpdateLayeredWindow` 先写入新像素, `SW_SHOW` 再显示窗口.
窗口从隐藏直接跳到新内容, 中间无 stale 帧, 所以无闪烁.

#### 圆形裁剪

```rust
// GDI path-based clipping
BeginPath(hdc);
Ellipse(hdc, 0, 0, diameter, diameter);
EndPath(hdc);
SelectClipPath(hdc, RGN_COPY);
// 此后所有绘制操作被裁剪到圆内
```

#### 彩虹描边

圆形描边需要新的 `circular_perimeter_position` 函数 (角度 → 0..1), 不能直接复用现有的矩形 `perimeter_position`. 实现: `atan2(y - cy, x - cx) / (2π)`.
- 描边宽度: 4px (固定)
- 两 pass 渲染: Pass 1 用圆形裁剪绘制放大内容; Pass 2 取消裁剪, 绘制描边 (描边不被裁剪)

#### 倍率文字

- 位置: 圆形底部居中
- 字体: 系统默认, 16px
- 颜色: 白色 + 黑色阴影 (可读性)
- 格式: "2.0x"

### Overlay Integration (overlay.rs)

```rust
struct App {
    // ... existing fields ...
    magnifier: Option<MagnifierWindow>,  // NEW
}
```

render() 末尾新增:
```rust
if self.state.magnifier_active {
    let mag = self.magnifier.get_or_insert_with(|| MagnifierWindow::new(350));
    let cursor_pos = get_cursor_pos();  // GetCursorPos Win32 API
    mag.render(cursor_pos, self.state.zoom_level, &self.color_mode, time_offset);
} else if let Some(mag) = &self.magnifier {
    mag.hide();
}
```

`magnifier.render()` 内部完成: 隐藏 → 捕获 → 放大 → 裁剪 → 描边 → 文字 → 提交 → 显示.
`magnifier_active` 已保证只在 modifier 按住时为 true (modifier release 重置为 false), 无需额外检查 modifier_is_held.

## Visual Design

```
┌─────────────────────────┐
│    ╭──────────────╮      │
│   ╱  放大内容(圆形) ╲     │
│  │                  │    │
│  │    鼠标位置      │    │
│  │     (中心)       │    │
│   ╲                ╱     │
│    ╰──────────────╯      │
│       ── 2.0x ──         │
│  彩虹描边边框 (4px)       │
└─────────────────────────┘
```

## Testing Strategy

### Unit Tests (state.rs)

1. `armed_digit_3_toggles_magnifier_active`
2. `drawing_digit_3_toggles_magnifier_active`
3. `idle_digit_3_is_noop`
4. `magnifier_scroll_up_increases_zoom`
5. `magnifier_scroll_down_decreases_zoom`
6. `magnifier_zoom_clamped_at_8_0`
7. `magnifier_zoom_clamped_at_1_5`
8. `scroll_without_magnifier_active_is_noop`
9. `modifier_release_resets_magnifier_active_preserves_zoom`
10. `escape_resets_magnifier_active`
11. `magnifier_and_pinned_independent`
12. `magnifier_and_spotlight_independent`

### Unit Tests (hook.rs)

1. `decide_keyboard_digit_3_modifier_held`
2. `decide_keyboard_digit_3_modifier_not_held_is_none`
3. `decide_mouse_scroll_up_modifier_held`
4. `decide_mouse_scroll_down_modifier_held`
5. `decide_mouse_scroll_modifier_not_held_is_none`

### Integration

- MagnifierWindow 创建/销毁不 panic
- BitBlt 捕获非空
- 圆形裁剪渲染不 panic
- 窗口跟随鼠标移动
