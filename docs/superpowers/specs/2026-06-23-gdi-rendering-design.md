# Spec: Replace softbuffer with GDI Direct Rendering

## Problem

Overlay window doesn't appear above GPU-accelerated apps (browsers, terminals) even though:
- Global input hooks work correctly (mouse is intercepted)
- `SetWindowPos(HWND_TOPMOST)` is called
- `set_visible(true)` is called

Root cause: winit's `with_transparent(true)` creates a WS_EX_LAYERED window using alpha-based transparency (`LWA_ALPHA`). Such windows are composited below DirectComposition visual trees used by GPU-accelerated apps. File Explorer (GDI-based) works fine because it doesn't use DirectComposition.

## Solution

Replace softbuffer with direct Win32 GDI rendering + `SetLayeredWindowAttributes(LWA_COLORKEY)` for transparency.

### How it works

1. **Remove winit transparency:** Remove `.with_transparent(true)` from window creation. This prevents winit from calling `DwmEnableBlurBehindWindow` and conflicting with our manual transparency setup.

2. **Color-key transparency:** Call `SetLayeredWindowAttributes(hwnd, COLOR_KEY, 0, LWA_COLORKEY)` AFTER `set_click_through()` has set `WS_EX_LAYERED`. `COLOR_KEY` (magenta `RGB(255, 0, 255)`) becomes fully transparent; all other colors are opaque.

3. **GDI drawing:** Replace existing `RedrawRequested` handler body with:
   - `GetDC(hwnd)` → get window DC
   - `CreatePen(PS_SOLID, BORDER_WIDTH, RED)` + `SelectObject` → set pen
   - `Rectangle(hdc, x0, y0, x1, y1)` → draw border in one call
   - `ReleaseDC` + `DeleteObject` → cleanup

4. **No back buffer needed:** The window is fullscreen with minimal drawing (just a border rectangle). No flicker because the transparent background is a color key, not actual painting.

### Changes

| File | Change |
|------|--------|
| `Cargo.toml` | Remove `softbuffer` dependency. Add `Win32_Graphics_Gdi` to `windows` crate features. |
| `overlay.rs` | Remove `.with_transparent(true)` from window creation. Remove `softbuffer` imports, `Context`, `Surface` fields. Call `SetLayeredWindowAttributes` after `set_click_through()`. Replace `render()` with GDI drawing in `RedrawRequested`. |

### Ordering

```
create_window(attrs)           // opaque window, no DWM blur
  ↓
set_click_through(&window)     // sets WS_EX_LAYERED | WS_EX_TRANSPARENT | ...
  ↓
set_layered_color_key(&window) // SetLayeredWindowAttributes(COLOR_KEY, LWA_COLORKEY)
```

`SetLayeredWindowAttributes` requires `WS_EX_LAYERED` to already be set. It must come AFTER `set_click_through()`.

### Color key

Use `RGB(255, 0, 255)` (magenta) as the transparent color key. All pixels with this exact color become see-through; everything else is opaque.

### Border drawing

Use `CreatePen` + `Rectangle` for a single-call 4px border:
```rust
let pen = CreatePen(PS_SOLID, BORDER_WIDTH, RGB(255, 0, 0));
let old_pen = SelectObject(hdc, pen.into());
let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH).into());
Rectangle(hdc, x0, y0, x1, y1);
SelectObject(hdc, old_pen);
SelectObject(hdc, old_brush);
DeleteObject(pen.into());
```

`NULL_BRUSH` ensures the interior is not filled (stays color-key = transparent).

### Testing

- Extract `normalize_rect` into testable pure function (already exists)
- Unit tests for edge cases: zero-area rect, negative coordinates, rect larger than screen
- Manual tests: Alt+drag in Explorer, Chrome, Windows Terminal

### Verification

1. `cargo test` — all tests pass
2. `cargo build --release` — compiles
3. Manual test: Alt+drag in File Explorer → red border appears
4. Manual test: Alt+drag in Chrome/Edge → red border appears
5. Manual test: Alt+drag in Windows Terminal → red border appears
6. Manual test: border is 4px, red, rectangular
7. Manual test: releasing mouse → border disappears
8. Manual test: mouse clicks pass through overlay (click-through)
