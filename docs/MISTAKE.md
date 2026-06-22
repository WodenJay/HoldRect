# MISTAKE.md

- GDI `FillRect`/`Rectangle` on `BI_RGB` 32-bit DIB: alpha 通道保持 0x00，DWM 当全透明处理，边框不可见。需要手动扫描像素设置 alpha=0xFF
- `UpdateLayeredWindow` 在 `windows` crate 中参数转换容易出错，用 `BitBlt` + `ValidateRect` 更可靠（和 softbuffer 一样）
- 隐藏窗口调用 `request_redraw()` 无效，Windows 不会给不可见窗口发 `WM_PAINT`
- `WS_EX_LAYERED` 必须在 `CreateWindowExW` 时设置（`with_transparent(true)`），不能事后用 `SetWindowLongPtrW` 添加
- `SetLayeredWindowAttributes(LWA_COLORKEY)` 搭配 `BitBlt` 到窗口 DC 可以实现透明，不需要 `UpdateLayeredWindow`
- `GetDC(None)` 获取屏幕 DC 用于创建兼容位图，`GetDC(hwnd)` 获取窗口 DC 用于 BitBlt，不要混用
- `windows` crate 0.58 中 `COLORREF` 在 `Foundation` 模块，不在 `Gdi`，需要单独 import
- `windows` crate 0.58 中 GDI 函数（`DeleteObject`、`SelectObject`）不需要 `.into()`，类型直接实现 `Param<HGDIOBJ>`
- `GetDC`/`ReleaseDC` 在 `windows` crate 0.58 中直接接受 `HWND`，不需要 `Some(hwnd)`
