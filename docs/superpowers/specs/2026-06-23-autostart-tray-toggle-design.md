# Design: 开机自启 via Tray Menu Toggle

> v0.2 最后一个 feature: 系统托盘菜单切换开机自启

## 目标

用户通过系统托盘右键菜单的复选项一键切换开机自启，无需手动编辑配置文件或注册表。

## 架构

### 新增 `src/autostart.rs` — 注册表操作

纯函数，与 tray 解耦:

```rust
// 注册表路径
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "HoldRect";

/// 检查当前 exe 路径是否已注册为开机自启
pub fn is_autostart_enabled() -> bool

/// 启用/禁用开机自启
pub fn set_autostart(enable: bool) -> Result<()>
```

实现细节:
- 使用 `windows::Win32::System::Registry` API
- **Cargo.toml 需新增 feature**: `"Win32_System_Registry"` (当前未包含)
- API: `RegOpenKeyExW` / `RegQueryValueExW` / `RegSetValueExW` / `RegDeleteValueW` / `RegCloseKey`
- 打开的 key handle 必须关闭 (RAII 或显式 `RegCloseKey`)
- `is_autostart_enabled()` 检查 value name "HoldRect" **是否存在** (不匹配路径，避免 exe 移动后误报)
- `set_autostart(false)` 删除时，`ERROR_FILE_NOT_FOUND` 视为成功 (值已不存在)
- exe 路径用引号包裹: `"\"C:\path\to\holdrect.exe\""`
- `#[cfg(windows)]` 门控
- 并发安全: 托盘菜单点击操作幂等，多次触发无副作用
- 测试: mock-free，直接操作真实注册表 (HKCU 用户级，不影响系统)

### 修改 `src/tray.rs` — 菜单增加 check item

```rust
// 导入方式: tray-icon 0.14 re-exports muda 的 menu 模块
use tray_icon::menu::{CheckMenuItem, PredefinedMenuItem};

// 现有: 退出 HoldRect
// 新增: 开机自启 ✓/✗ (CheckMenuItem)

let autostart_item = CheckMenuItem::new("开机自启", true, is_autostart_enabled(), None);
let separator = PredefinedMenuItem::separator();
let quit_item = MenuItem::new("退出 HoldRect", true, None);

let tray_menu = Menu::new();
tray_menu.append(&autostart_item);
tray_menu.append(&separator);
tray_menu.append(&quit_item);
```

菜单事件处理:
- autostart_item 点击: toggle `set_autostart(!is_autostart_enabled())`, 更新 checked 状态
- 失败静默处理，不崩溃

### 不改动的模块

- `main.rs` — 无变化
- `overlay.rs` — 无变化
- `config.rs` — 无变化 (开机自启状态不在 config.toml 中)
- `hook.rs` / `state.rs` — 无变化

## 测试策略

### autostart.rs 测试
- `set_autostart(true)` → `is_autostart_enabled() == true`
- `set_autostart(false)` → `is_autostart_enabled() == false`
- 幂等性: 重复 enable/disable 不报错
- exe 路径含空格时正确加引号
- stale path 覆盖: 注册表已有旧路径值时，`set_autostart(true)` 覆盖为当前路径
- disable 不存在的值: `set_autostart(false)` 当值已不存在时成功 (幂等)

### tray.rs 测试
- `CheckMenuItem` 创建成功
- 初始状态从注册表正确读取
