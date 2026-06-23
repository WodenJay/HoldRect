# Design: Pinned Mode (多框共存)

> 数字键 `1` toggle pinned 模式, 松手后框冻结在屏幕上, 多框共存, Esc 清除所有。

**Scope:** 仅 Pinned mode (digit key `1`). Spotlight (digit key `2`), 状态提示弹窗, 快捷键说明书, 托盘菜单增强 — 各自单独设计迭代。

---

## 1. State Machine (state.rs)

### New InputEvent Variants

```rust
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    DigitPressed(u8),       // digit key pressed while modifier held (currently only `1`)
    EscapePressed,           // Esc key
}
```

### AppState Extension

```rust
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<(i32, i32, i32, i32)>,  // normalized (x0, y0, x1, y1)
    pub pinned_active: bool,  // per-rect toggle, reset after MouseUp (PRD: "画新框时重置")
}
```

### State Transitions

`DigitPressed(1)` toggles `pinned_active` when modifier is held (i.e. in Armed or Drawing state — both imply modifier held; Idle does not). Only key `1` is handled; other digit keys are ignored.

| Current State | Event | pinned_active | Action |
|---|---|---|---|
| Armed (modifier held) | `DigitPressed(1)` | toggle | flip `pinned_active` |
| Drawing (modifier held) | `DigitPressed(1)` | toggle | flip `pinned_active` |
| Drawing | `MouseButtonUp` | true | push normalized rect to `pinned_rects`, → Armed, **`pinned_active = false`** |
| Drawing | `MouseButtonUp` | false | → Armed (existing behavior, rect disappears) |
| Drawing | `EscapePressed` | any | → Armed, current rect discarded, **`pinned_active = false`** |
| Drawing | `ModifierChanged { pressed: false }` | true | push rect to `pinned_rects`, → Idle, `pinned_active = false` |
| Drawing | `ModifierChanged { pressed: false }` | false | → Idle (existing behavior) |
| Armed | `ModifierChanged { pressed: false }` | any | → Idle, `pinned_active = false` |
| Armed | `EscapePressed` | - | `pinned_rects.clear()`, no state change |
| Idle | `EscapePressed` | - | `pinned_rects.clear()`, no state change |

**Key invariants:**
- `pinned_active` resets to `false` after each `MouseButtonUp` (per-rect, as PRD requires "画新框时数字键状态重置为默认")
- `pinned_active` also resets on modifier release (safety net)
- `pinned_rects` persists until `EscapePressed`

---

## 2. Hook (hook.rs)

### `decide_keyboard` Signature Change

```rust
pub(crate) fn decide_keyboard(
    vk_code: u32,
    is_key_down: bool,
    modifier_codes: &[u32],
    modifier_held: bool,  // new: SHOULD_SUPPRESS value
) -> Option<InputEvent>
```

### Logic

```
1. If vk_code in modifier_codes → ModifierChanged (existing)
2. If is_key_down AND modifier_held:
   a. vk_code == 0x31 (VK_1) → DigitPressed(1)
3. If is_key_down AND vk_code == VK_ESCAPE → EscapePressed (regardless of modifier_held)
4. Otherwise → None
```

Only digit `1` is handled now. When Spotlight (digit `2`) is added in a later iteration, extend step 2a.

Esc is NOT gated on modifier_held — user can press Esc anytime to clear pinned rects (PRD does not require modifier for Esc).

### Call Site Change

```rust
// keyboard_hook_proc
let modifier_held = SHOULD_SUPPRESS.load(Ordering::Relaxed);
if let Some(event) = decide_keyboard(kb.vkCode, is_key_down, &modifier_codes, modifier_held) {
    // ... existing send logic
}
```

---

## 3. Overlay Rendering (overlay.rs)

### DIB Refactor

Current `draw_border` creates/overwrites entire DIB + calls `UpdateLayeredWindow` per invocation. This breaks with multiple rects (each overwrites the previous).

**Split into three functions:**

1. `clear_dib(dib_cache: &mut Option<DibCache>)` — fill with transparent pixels
2. `draw_rect_in_dib(dib_cache, start, end, border_width, color_mode, time_offset)` — draw one rect into existing DIB
3. `commit_dib(window, dib_cache)` — call `UpdateLayeredWindow` to push to screen

### Render Method

```rust
fn render(&mut self) {
    let Some(window) = &self.window else { return; };

    let has_drawing = matches!(&self.state.drawing, DrawingState::Drawing { .. });
    let has_pinned = !self.state.pinned_rects.is_empty();

    if !has_drawing && !has_pinned {
        window.set_visible(false);
        hide_from_alt_tab(window);
        return;
    }

    show_window_topmost(window);
    clear_dib(&mut self.dib_cache);

    let time_offset = compute_time_offset();

    // Draw all pinned rects
    for &(x0, y0, x1, y1) in &self.state.pinned_rects {
        draw_rect_in_dib(&mut self.dib_cache, (x0, y0), (x1, y1),
                         self.border_width, &self.color_mode, time_offset);
    }

    // Draw active rect on top
    if let DrawingState::Drawing { start, current } = &self.state.drawing {
        draw_rect_in_dib(&mut self.dib_cache, *start, *current,
                         self.border_width, &self.color_mode, time_offset);
    }

    commit_dib(window, &mut self.dib_cache);
}
```

### `about_to_wait` Simplification

```rust
fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    while let Ok(event) = self.input_rx.try_recv() {
        let new_state = process_event(&self.state, &event);
        self.state = new_state;
    }

    self.render();

    let needs_animation = matches!(&self.state.drawing, DrawingState::Drawing { .. })
        || !self.state.pinned_rects.is_empty();

    if needs_animation {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    } else {
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}
```

---

## 4. Files Changed

| File | Change |
|---|---|
| `src/state.rs` | Add `DigitPressed`, `EscapePressed` to `InputEvent`. Add `pinned_rects`, `pinned_active` to `AppState`. New transition logic. |
| `src/hook.rs` | Extend `decide_keyboard` with `modifier_held` param + digit/Esc handling. |
| `src/overlay.rs` | Refactor `draw_border` → `clear_dib` + `draw_rect_in_dib` + `commit_dib`. New `render()` with multi-rect support. Simplified `about_to_wait`. |
| `src/main.rs` | No changes needed (existing config pass-through sufficient). |

---

## 5. Testing Strategy

- **state.rs**: Unit tests for all new transitions (DigitPressed toggle in Armed/Drawing, pinned on mouse up + reset, modifier release reset, Esc cancel draw, Esc clear pinned_rects, multi-rect accumulate, per-rect reset between consecutive draws).
- **hook.rs**: Unit tests for `decide_keyboard` with digit `1` + Esc + modifier_held combinations, other digit keys ignored.
- **overlay.rs**: `hsv_to_rgb` and `perimeter_position` already tested. DIB functions are Win32-coupled — manual verification.
