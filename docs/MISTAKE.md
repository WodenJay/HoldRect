# MISTAKE.md

> 记录开发过程中频繁出现的错误和教训，避免重复踩坑。只记录关键的, 不要把小错误也记录进来, **过多的记录=没有记录!**

## 格式

每条记录包含:
- **日期**: 首次出现时间
- **问题**: 错误描述
- **根因**: 为什么会犯这个错
- **修复**: 怎么修的
- **预防**: 怎么避免再犯

---

## 2026-06-22: rdev 0.5 has no `mouse_coords()` API

- **问题**: Brief code called `rdev::mouse_coords()` which doesn't exist in rdev 0.5.3
- **根因**: rdev has no standalone mouse position query function. Coordinates only appear inside `EventType::MouseMove { x, y }`. `ButtonPress`/`ButtonRelease` carry only the Button enum, no coordinates
- **修复**: Track last position in `static Mutex<(f64, f64)>`, update on MouseMove, read on Button events
- **预防**: Always check actual crate API with `cargo doc` before trusting brief/example code verbatim
