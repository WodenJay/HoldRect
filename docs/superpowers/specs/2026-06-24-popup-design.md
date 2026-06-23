# Popup System Design Spec

> Status popup + Cheatsheet popup — Apple-style, spring animation, GDI rendering.

---

## 1. Architecture

### Popup Window Strategy

One persistent hidden popup window. Show/update when needed, hide when done. Avoids repeated window creation/destruction.

**Popup window styles**: `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW`. The `WS_EX_TRANSPARENT` flag ensures mouse events pass through — critical for status popup which appears during drag. `WS_EX_TOOLWINDOW` prevents it from appearing in Alt-Tab.

```
App (winit event loop)
├── overlay_window  (fullscreen transparent, existing)
├── popup_window    (small, WS_EX_TRANSPARENT mouse-passthrough, visible only when active) [NEW]
│
├── popup_manager: PopupManager  [NEW]
│   ├── active_popup: Option<PopupContent>
│   ├── anim_state: AnimationState
│   └── timer: PopupTimer
│
└── popup_renderer: GdiRenderer  [NEW]
    ├── mem_dc: HDC (memory DC for double-buffering)
    ├── mem_bitmap: HBITMAP
    ├── font_normal: HFONT (Segoe UI 14px)
    ├── font_key: HFONT (Segoe UI 13px semibold)
    ├── font_desc: HFONT (Segoe UI 13px regular)
    └── bg_brush: HBRUSH
```

### GDI Initialization (once, at popup window creation)

1. `CreateCompatibleDC` -> memory DC for off-screen rendering
2. `CreateDIBSection` -> BGRA pixel buffer matching popup size
3. `CreateFontW` x3 -> Segoe UI at specified sizes/weights
4. Render pipeline: clear -> `RoundRect` background -> `DrawTextW` text -> `BitBlt` to window

Shadow: layered semi-transparent `RoundRect` drawn offset behind the card (~2-3 layers at increasing alpha).

### Cross-Platform Strategy (v0.3)

```
src/popup/
    mod.rs          — PopupManager, AnimationState, PopupContent (shared)
    renderer.rs     — trait Renderer { draw_card(), draw_text() }
    gdi_renderer.rs — GDI implementation (#[cfg(windows)])
    // renderer_mac.rs — CoreGraphics (v0.3)
    // renderer_lin.rs — Cairo (v0.3)
```

> **Cross-platform note**: GDI is Windows-only. v0.3 will replace with platform-specific renderers behind the `Renderer` trait. Popup logic (animation, state, layout) is platform-independent. v0.3 renderer files will likely be simpler than GDI — CoreGraphics and Cairo both have native rounded rect + text APIs.

---

## 2. Spring Animation

Critically-damped spring with slight overshoot for Apple-like feel.

### Timing

```
Y position
  ^
  |  +-- start (offscreen -60px)
  |  |
  |  +.
  |    +.         overshoot ~8px
  |      +--------------+
  |                     +---------- end (y=0)
  |
  +--+-----------------------------+---> time
     0    150ms   300ms   500ms

  slide-in:  ~400ms (with overshoot bounce)
  hold:      1000ms    // ponytail: overrides PRD's 0.5s per user preference for longer reading time
  slide-out: ~300ms (no overshoot, crisp exit)
  total:     ~1.7s
```

### Spring Parameters

- Slide-in: `omega_n = 18.0`, `zeta = 0.82` (slightly underdamped -> ~4% overshoot)
- Slide-out: `omega_n = 22.0`, `zeta = 1.0` (critically damped, no bounce)

### State Machine

```
Hidden -> SlidingIn -> Holding -> SlidingOut -> Hidden
              ^                  |
              +-- repeated toggle -+ (reset hold timer, content updates in place)
```

Key behavior: continuous toggles during slide-in do NOT interrupt animation. Only reset hold timer and update text. Position stays smooth, no jumping.

### Animation Implementation

Pure function, testable:

```rust
fn spring_position(elapsed_secs: f64, omega_n: f64, zeta: f64, target: f64) -> f64
```

Returns Y offset. At t=0, returns start position (-60). As t->infinity, returns 0 (target).

---

## 3. Visual Design

### Status Popup — single-line card

```
            screen top
    +---------------------+  <- 48px from top
    |                     |
    |  Pinned · Spotlight |  <- Segoe UI 14px, #FFFFFF, weight 500
    |                     |
    +---------------------+

    width:  24px padding + text width + 24px (auto-sizing)
    height: 44px
    border-radius: 12px
    background: rgba(28, 28, 30, 0.88)     <- Apple dark card
    shadow: layered RoundRect (2-3 layers, offset +2px, rgba(0,0,0,0.1-0.3))
    horizontal: centered on current screen
```

### Cheatsheet Popup — two-column list

```
    +-----------------------------------+
    |                                   |
    |   Alt + drag              Draw    |  <- key: 13px semibold #E5E5E5
    |   1                     Pin      |  <- desc: 13px regular #AEAEB2
    |   2                     Spotlight |
    |   Esc                   Clear    |
    |   Alt + `               Help     |
    |                                   |
    +-----------------------------------+

    width:  ~320px (fixed)
    height: 20px padding + rows * 32px + 20px
    border-radius: 14px
    background: rgba(28, 28, 30, 0.92)      <- slightly more opaque than status
    shadow: layered RoundRect (3-4 layers, offset +4px, rgba(0,0,0,0.1-0.4))
    position: centered on current screen (both H and V)
    key column: left-aligned, 24px left padding
    desc column: right-aligned, 24px right padding
```

### Typography

- Font: Segoe UI (Windows system font, zero extra binary size)
- Status text: 14px, FW_MEDIUM (500), `#FFFFFF`
- Cheatsheet key: 13px, FW_SEMIBOLD (600), `#E5E5E5`
- Cheatsheet desc: 13px, FW_NORMAL (400), `#AEAEB2` (Apple secondary gray)

### Not Included (deliberate design decision)

The PRD specifies icons (📌, 🔦) in status text. User explicitly chose to exclude them for a cleaner, more premium aesthetic. No emojis, no icon fonts — pure typography.

- No icons / emojis (overrides PRD — user decision)
- No section headers / dividers
- No gradient backgrounds
- No key badge / pill shapes
- No border outlines

Pure typography, differentiated by weight and color hierarchy.

---

## 4. Popup Behavior

### Status Popup (digit key toggle)

**Trigger**: `pinned_active` or `spotlight_active` changes in AppState
**Position**: cursor's monitor, horizontally centered, 48px from top

**Display content**:

| pinned_active | spotlight_active | Display |
|---|---|---|
| false | false | `Transient` |  // ponytail: PRD says "show Transient or don't show"; we always show
| true | false | `Pinned` |
| false | true | `Spotlight` |
| true | true | `Pinned · Spotlight` |

**Real-time update rules**:
- Continuous toggles within 0.3s -> text updates immediately, hold timer resets
- Animation position does not jump (toggle during slide-in updates text in place)
- From SlidingOut -> reverse to SlidingIn (re-enter). Spring restarts from current Y position (not from -60px), so no visual jump.

**Lifecycle**:
```
DigitPressed -> PopupManager::show_status(text)
  +-- Hidden? -> start SlidingIn
  +-- SlidingIn/Holding? -> update text, reset hold timer
  +-- SlidingOut? -> reverse to SlidingIn

hold timer expires -> SlidingOut
slide-out complete -> Hidden
```

### Cheatsheet Popup (modifier + backtick hold)

**Trigger**: modifier + VK_OEM_3 (backtick) both held
**Position**: cursor's monitor, centered (both H and V)

**Display content** (static, built at startup):
```
Alt + drag          Draw
1                   Pin
2                   Spotlight
Esc                 Clear
Alt + `             Help
```

**Lifecycle**:
```
modifier + ` pressed -> PopupManager::show_cheatsheet()
  +-- Hidden? -> SlidingIn
  +-- already showing? -> no-op

modifier or ` released -> PopupManager::hide_cheatsheet()
  +-- SlidingIn/Holding? -> SlidingOut
```

No hold timer. Pure hold-to-show behavior. Mutually exclusive with status popup — cheatsheet showing suppresses status popup triggers.

### New InputEvents

```rust
InputEvent::ToggleHelp,   // modifier + ` pressed
InputEvent::HideHelp,     // modifier or ` released
```

Detection in `decide_keyboard`: check VK_OEM_3 combined with modifier state.

> **Layout limitation**: VK_OEM_3 (0xC0) is the backtick key on US layout. On other layouts (e.g. German, French), this physical key produces a different character. This is a known limitation; the shortcut uses the physical key position, not the character. Same approach as VS Code and other tools.

### Multi-Monitor

Use `GetCursorPos` + `MonitorFromPoint` + `GetMonitorInfoW` to get current screen work area:
- Status popup: `work_area.left + (work_area.width - popup_width) / 2`, `work_area.top + 48`
- Cheatsheet popup: both H and V centered on work area

Popup position computed at show time. Does not track mouse movement.

---

## 5. State Integration

### PopupState (separate from AppState)

Popup state does not mix into AppState. AppState manages drawing logic, PopupManager manages UI feedback.

```rust
// src/popup/mod.rs

pub enum PopupContent {
    Status { text: String },
    Cheatsheet,
}

pub enum PopupPhase {
    Hidden,
    SlidingIn { started_at: Instant },
    Holding { started_at: Instant },
    SlidingOut { started_at: Instant },
}

pub struct PopupManager {
    content: Option<PopupContent>,
    phase: PopupPhase,
    status_text: String,
    cheatsheet_rows: Vec<(String, String)>,  // pre-built
    target_monitor_rect: (i32, i32, i32, i32),  // work area
}
```

### Event Flow

```
InputEvent::DigitPressed(1)
    |
    +-> state::process_event()     <- existing logic, updates AppState
    |
    +-> popup_manager.on_digit_toggle(&new_state)
            |
            +-- build status_text (from new_state.pinned_active + spotlight_active)
            +-- show_status(text) or update_status(text)
            +-- trigger popup window redraw

InputEvent::ToggleHelp
    |
    +-> popup_manager.show_cheatsheet()

InputEvent::HideHelp
    |
    +-> popup_manager.hide_cheatsheet()
```

### about_to_wait Modification

```rust
fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    // Existing: poll input channel
    while let Ok(event) = self.input_rx.try_recv() {
        self.state = process_event(&self.state, &event);
        self.popup_manager.on_event(&event, &self.state);  // NEW
    }

    // Existing: overlay render
    self.render();

    // NEW: popup animation tick + render
    if self.popup_manager.needs_frame() {
        self.popup_manager.tick();
        self.popup_renderer.render(&self.popup_manager, &self.popup_window);
    }

    // Existing: window visibility
    ...
}
```

### Popup vs Overlay Independence

| Scenario | Overlay Window | Popup Window |
|---|---|---|
| Idle | hidden | hidden |
| Drawing | visible | hidden (unless toggle) |
| Drawing + toggle | visible | visible (status popup) |
| Modifier + ` held | hidden | visible (cheatsheet) |
| Pinned frozen | visible | hidden |

Two windows, fully independent visibility.

### File Structure (new files)

```
src/popup/
    mod.rs          — PopupManager, PopupContent, PopupPhase
    renderer.rs     — trait PopupRenderer
    gdi_renderer.rs — GDI implementation (#[cfg(windows)])
    animation.rs    — Spring animation math (pure functions, testable)
```

---

## 6. Testing Strategy

### animation.rs — pure function tests

- `spring_position(0, ...) == start_offset`
- `spring_position(infinity, ...) == 0`
- `spring_position` overshoots target by ~4% at peak
- `spring_position` converges within 500ms
- Slide-out has no overshoot (zeta=1.0)

### PopupManager — state machine tests

- `show_status` from Hidden -> SlidingIn
- `show_status` from SlidingIn -> updates text, resets timer
- `show_status` from SlidingOut -> reverses to SlidingIn
- `tick` advances phase: SlidingIn -> Holding (after anim duration)
- `tick` advances phase: Holding -> SlidingOut (after hold duration)
- `tick` advances phase: SlidingOut -> Hidden (after anim duration)
- `show_cheatsheet` suppresses status popup
- `hide_cheatsheet` triggers SlidingOut
- `on_digit_toggle` builds correct status text from AppState flags

### Content text generation tests

- `(false, false)` -> `"Transient"`
- `(true, false)` -> `"Pinned"`
- `(false, true)` -> `"Spotlight"`
- `(true, true)` -> `"Pinned · Spotlight"`

### Rendering — manual / integration

- GDI initialization (CreateCompatibleDC, CreateDIBSection, CreateFontW) succeeds
- Popup window created with correct styles
- Multi-monitor: popup appears on cursor's screen
- Text renders legibly
- Rounded corners visible (GDI RoundRect)
- Shadow visible (layered RoundRect)
