# MISTAKE.md

- GDI `FillRect`/`Rectangle` on `BI_RGB` 32-bit DIB: alpha 通道保持 0x00，DWM 当全透明处理，边框不可见。需要手动扫描像素设置 alpha=0xFF
- `LWA_COLORKEY` + `BitBlt` 在 winit layered window 上不可靠，品红背景无法变透明。必须用 `UpdateLayeredWindow` + per-pixel alpha
- `UpdateLayeredWindow` 在 `windows` crate 0.58 中 `hdcdst`/`hdcsrc` 是 raw `HDC`，不是 `Option<HDC>` — 直接传值，不要包 `Some()`
- `decide_keyboard` 必须识别 `VK_MENU`(0x12) 通用 Alt 键，浏览器和终端经常上报此键码而非 `VK_LMENU`/`VK_RMENU`
- `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` 会破坏 `SetLayeredWindowAttributes` 颜色键匹配，品红无法被识别为透明色
- 隐藏窗口调用 `request_redraw()` 无效，Windows 不会给不可见窗口发 `WM_PAINT`
- `WS_EX_LAYERED` 可以通过 `SetWindowLongPtrW` 事后添加（用于 `UpdateLayeredWindow`），不需要 `with_transparent(true)`
- 直接用像素写 BGRA + `UpdateLayeredWindow` 比 GDI `Rectangle` + alpha 修复更可靠，避免 GDI alpha 通道问题
- `GetDC(HWND::default())` 获取屏幕 DC，用于 `UpdateLayeredWindow` 的 `hdcDst`；不要用 `GetDC(hwnd)`
- `windows` crate 0.58 中 `COLORREF` 在 `Foundation` 模块，不在 `Gdi`，需要单独 import
- `windows` crate 0.58 中 GDI 函数（`DeleteObject`、`SelectObject`）不需要 `.into()`，类型直接实现 `Param<HGDIOBJ>`
- `GetDC`/`ReleaseDC` 在 `windows` crate 0.58 中直接接受 `HWND`，不需要 `Some(hwnd)`
- `f32` 对 Unix 时间戳（~1.7×10⁹）精度不足，`as_secs_f32()` 会丢失小数部分，导致 `time % 1.0 == 0.0`。彩虹动画必须用 `as_secs_f64()` + `.fract()` 取小数部分再转 `f32`
- 曼哈顿颜色距离求和后 `as u8` 截断：蓝/紫色像素距离 >255 溢出回绕，被误判为背景色而变透明。颜色距离用 `u16` 或更大类型
- `CheckMenuItem::new(text, enabled, checked, accelerator)` 第2个参数是 `enabled`（是否可点击），第3个是 `checked`（是否勾选）。顺序搞反会导致菜单灰化或默认勾选异常
