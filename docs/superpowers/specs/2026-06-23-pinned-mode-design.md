# Design: Pinned Mode (多框共存)

> 数字键 `1` toggle pinned 模式, 松手后框冻结在屏幕上, 多框共存, Esc 清除所有。

---

## 1. State Machine (state.rs)

### New InputEvent Variants

```rust
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    DigitPressed(u8),       // digit key 1-9, only when modifier held
    EscapePressed,           // Esc key
}
```

### AppState Extension

```rust
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<(i32, i32, i32, i32)>,  // normalized (x0, y0, x1, y1)
    pub pinned_active: bool,  // per-gesture toggle, reset on modifier release
}
```

### State Transitions

| Current State | Event | pinned_active | Action |
|---|---|---|---|
| Any (modifier held) | `DigitPressed(1)` | toggle | flip `pinned_active` |
| Drawing | `MouseButtonUp` | true | push normalized rect to `pinned_rects`, → Armed |
| Drawing | `MouseButtonUp` | false | → Armed (existing behavior, rect disappears) |
| Drawing | `ModifierChanged { pressed: false }` | true | push rect to `pinned_rects`, → Idle, `pinned_active = false` |
| Drawing | `ModifierChanged { pressed: false }` | false | → Idle (existing behavior) |
| Armed | `ModifierChanged { pressed: false }` | any | → Idle, `pinned_active = false` |
| Any | `EscapePressed` | - | `pinned_rects.clear()`, no state change |

**Key invariant:** `pinned_active` resets to `false` on modifier release. `pinned_rects` persists until Esc.

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
   a. vk_code in 0x31..0x39 → DigitPressed(vk_code - 0x30)
   b. vk_code == VK_ESCAPE → EscapePressed
3. Otherwise → None
```

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

- **state.rs**: Unit tests for all new transitions (DigitPressed toggle, pinned on mouse up, modifier release reset, Esc clear, multi-rect accumulate).
- **hook.rs**: Unit tests for `decide_keyboard` with digit/Esc + modifier_held combinations.
- **overlay.rs**: `hsv_to_rgb` and `perimeter_position` already tested. DIB functions are Win32-coupled — manual verification.
