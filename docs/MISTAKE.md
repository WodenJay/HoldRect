# MISTAKE.md - Frequent Errors and Lessons

## GDI + Layered Window: Alpha Channel

**Problem:** GDI operations (`FillRect`, `Rectangle`) on a DIB section with `BI_RGB` format only set RGB channels. Alpha channel stays at 0x00. DWM treats alpha=0 as fully transparent, making all GDI-drawn pixels invisible on layered windows.

**Root cause:** `CreateDIBSection` with `BI_RGB` + `biBitCount=32` initializes alpha to 0. GDI doesn't touch alpha. DWM reads ARGB and uses alpha for compositing.

**Fix:** After GDI drawing, scan bitmap pixels and set alpha=0xFF on border pixels:
```rust
// BORDER_COLOR_GDI = 0x000000FF → BGRA: B=0x00, G=0x00, R=0xFF
for y in 0..h as usize {
    let row = bits.add(y * row_bytes);
    for x in 0..w as usize {
        let px = row.add(x * 4);
        if *px.add(0) == 0x00 && *px.add(1) == 0x00 && *px.add(2) == 0xFF {
            *px.add(3) = 0xFF; // alpha = opaque
        }
    }
}
```

**Lesson:** softbuffer avoids this by using `BI_BITFIELDS` format where alpha defaults to 0xFF in the custom layout.

---

## UpdateLayeredWindow E_INVALIDARG (0x80070057)

**Problem:** `UpdateLayeredWindow` fails with `E_INVALIDARG` when called from the `windows` crate.

**Root cause:** Multiple parameter issues:
1. `hdcdst` must be the screen DC from `GetDC(NULL)`, not `HDC::default()` (NULL)
2. `pptDst` as `Some(&origin as *const POINT)` may not work with all `Param<HDC>` conversions
3. The function is extremely sensitive to bitmap format — must be 32-bit ARGB with proper alpha

**Fix:** Avoid `UpdateLayeredWindow` entirely. Use `BitBlt` to window DC + `ValidateRect` instead (same as softbuffer).

---

## RedrawRequested Never Fires on Hidden Windows

**Problem:** `window.request_redraw()` called but `WindowEvent::RedrawRequested` never fires.

**Root cause:** Windows doesn't deliver `WM_PAINT` to invisible windows. `request_redraw()` posts `WM_PAINT` which is silently dropped.

**Fix:** Call rendering directly from `about_to_wait` instead of relying on `RedrawRequested`.

---

## WS_EX_LAYERED Must Be Set at Creation Time

**Problem:** `SetLayeredWindowAttributes(LWA_COLORKEY)` doesn't work when `WS_EX_LAYERED` is added via `SetWindowLongPtrW` after window creation.

**Root cause:** On Windows 10/11, `SetLayeredWindowAttributes` requires `WS_EX_LAYERED` to be set at `CreateWindowExW` time, not added later.

**Fix:** Use winit's `with_transparent(true)` which sets `WS_EX_LAYERED` during window creation.

---

## SetLayeredWindowAttributes vs UpdateLayeredWindow

**Problem:** Confusion about which function to use for layered window transparency.

**Clarification:**
- `SetLayeredWindowAttributes(LWA_COLORKEY)`: Sets color-key rule. DWM applies it when reading the window's GDI surface. Works with `BitBlt` to window DC.
- `UpdateLayeredWindow`: Replaces the entire window surface with a bitmap. Required when GDI operations don't reach DWM's redirection surface. Hard to get right with the `windows` crate.

**Best approach:** `SetLayeredWindowAttributes(LWA_COLORKEY)` + `BitBlt` to window DC + `ValidateRect`. This matches how softbuffer works.
