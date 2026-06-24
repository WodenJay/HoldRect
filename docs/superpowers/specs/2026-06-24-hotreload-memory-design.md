# Hot-reload Config + Memory Optimization Design

> v0.4 迭代: 热加载配置 + 内存占用优化

---

## 1. Summary

两个独立特性, 一个迭代完成:

1. **热加载配置**: 用户修改 `~/.holdrect/config.toml` 后无需重启, 配置立即生效
2. **内存优化**: 测量当前占用, 通过编译优化和运行时优化降低常驻内存

---

## 2. Hot-reload Config

### 2.1 Architecture

```
~/.holdrect/config.toml 变更
    │
    ▼
watcher线程 (ReadDirectoryChangesW)
    │ 检测到修改事件, 读取+解析config
    │
    ▼
mpsc::channel<AppConfig>
    │
    ▼
main event loop (about_to_wait中polling)
    │
    ├─ 更新 MODIFIER_CODES (RwLock<Vec<u32>>)
    └─ 更新 App中的 border_width / color_mode / modifier_name
```

### 2.2 File Changes

| File | Change |
|------|--------|
| `src/main.rs` | 新增config watcher线程启动; 新增config change channel `(config_tx, config_rx)` |
| `src/overlay.rs` | `App`结构体持有`config_rx`; `about_to_wait`中`try_recv`; 从App字段读取渲染参数 |
| `src/hook.rs` | `MODIFIER_CODES`从`OnceLock<Vec<u32>>`改为`RwLock<Vec<u32>>`; 新增`update_modifier_codes()` |
| `src/config.rs` | `AppConfig`加`PartialEq`; 新增`watch_config_dir()`函数 |
| `Cargo.toml` | 不加新依赖 |

### 2.3 Watcher Thread

使用Win32 `ReadDirectoryChangesW` 同步I/O 监听 `~/.holdrect/` 目录:

- 如果 `~/.holdrect/` 目录不存在, watcher线程静默退出(跟当前config加载行为一致)
- 如果 `~/.holdrect/` 存在但 `config.toml` 不存在, 监听目录等待文件创建
- 编辑器保存文件可能触发多次事件(rename + modify), 用简单去抖: 收到事件后sleep 100ms再读取

```rust
fn watch_config_dir(dir: PathBuf, tx: Sender<AppConfig>) {
    // 0. 如果目录不存在, 静默退出
    // 1. CreateFileW 打开目录, FILE_LIST_DIRECTORY | SYNCHRONIZE, FILE_FLAG_BACKUP_SEMANTICS
    // 2. 分配 FILE_NOTIFY_INFORMATION buffer
    // 3. Loop (同步I/O, ReadDirectoryChangesW阻塞直到有变更):
    //    - ReadDirectoryChangesW (同步模式, 无OVERLAPPED, 函数阻塞直到事件到达)
    //    - 解析 FILE_NOTIFY_INFORMATION, 过滤 config.toml
    //    - sleep 100ms (去抖)
    //    - fs::read_to_string → AppConfig::parse()
    //    - 成功: tx.send(new_config)
    //    - 失败: eprintln! 警告, 保持当前配置
}
```

### 2.4 hook.rs 变更

```rust
// Before:
static MODIFIER_CODES: OnceLock<Vec<u32>> = OnceLock::new();

// After:
static MODIFIER_CODES: std::sync::RwLock<Vec<u32>> = ...;

pub fn update_modifier_codes(new_codes: Vec<u32>) {
    *MODIFIER_CODES.write().unwrap() = new_codes;
}
```

`keyboard_hook_proc` 中读取改为 `MODIFIER_CODES.read().unwrap()`。读锁性能影响可忽略。

RwLock poisoning: 如果任何线程在持锁时panic, `unwrap()`会传播panic。当前代码中不存在`catch_unwind`(已验证), panic即进程终止, poisoning无实际影响。使用`unwrap()`保持简单。

### 2.5 overlay.rs 变更

`App` 结构体新增字段:

```rust
pub struct App {
    // ...existing...
    border_width: i32,
    color_mode: ColorMode,
    modifier_name: String,
    config_rx: Receiver<AppConfig>,
}
```

`about_to_wait` 中非阻塞 poll:

```rust
while let Ok(new_config) = self.config_rx.try_recv() {
    self.border_width = new_config.border_width;
    self.color_mode = new_config.color_mode;
    self.modifier_name = new_config.modifier_name;
    crate::hook::update_modifier_codes(new_config.modifier_vk_codes);
}
```

### 2.6 Hot-reloadable Fields

| Field | Hot-reloadable | Note |
|-------|---------------|------|
| `modifier_vk_codes` | ✅ | RwLock swap, immediate |
| `border_width` | ✅ | Next frame uses new value |
| `color_mode` | ✅ | Next frame uses new value |
| `modifier_name` | ✅ | Used in popup display, PopupManager cheatsheet rows rebuilt on change |

### 2.6.1 Cascade Updates

`PopupManager` 在 `new()` 时从 `modifier_name` 构建 `cheatsheet_rows`。热更新 `modifier_name` 时, 需要在 `about_to_wait` 中同步更新 PopupManager 的 cheatsheet_rows。方案: `PopupManager` 新增 `update_modifier_name(&mut self, name: &str)` 方法, 重建 cheatsheet_rows。

### 2.7 Error Handling

- Config file deleted → keep current config, stderr warning
- Config file has syntax error → keep current config, stderr warning with error detail
- Config file has partial valid fields (e.g. `modifier = "Ctrl"` valid but `border_width = "abc"` invalid) → 当前`AppConfig::parse`行为: 整个解析失败, 回退到默认值。热加载场景下应改为: 保持当前配置不变(不回退到默认), 因为用户已有运行中的合理配置, 不应因为一个字段错误就全部重置。实现: `watch_config_dir`中比较新config与当前config, 只有完整解析成功才发送。

### 2.8 Testing Strategy

1. **`config::watch_config_dir` unit test**: 模拟目录变更, 验证channel收到新config
2. **`hook::update_modifier_codes` test**: 验证RwLock写入后读取返回新值
3. **`overlay` config poll test**: 模拟config_rx发送, 验证App字段更新
4. **Integration test**: 写入临时config文件, 验证端到端热更新
5. **Error test**: 写入无效TOML, 验证配置保持不变

---

## 3. Memory Optimization

### 3.1 Approach

数据驱动: 先测量 → 编译优化(免费) → 运行时优化(按需) → 验证

### 3.2 Step 1: Baseline Measurement (必须最先执行)

编译当前release版本(无任何优化改动), 启动后用 `GetProcessMemoryInfo` API 测量:
- Working Set Size (物理内存占用)
- Pagefile Usage (提交内存)

输出baseline数字, 记录在测试日志中。后续所有优化都与此对比。

实现: 写一个`--mem-report`命令行flag, 启动后调用`GetProcessMemoryInfo`打印内存指标然后退出。可复用于CI回归检测。

### 3.3 Step 2: Compile-time Optimization

```toml
# Cargo.toml
[profile.release]
lto = true              # 链接时优化, 减少死代码
strip = true            # 去掉符号表和调试信息
panic = "abort"         # 不展开栈, 减少panic infrastructure (无catch_unwind, 安全)
codegen-units = 1       # 更好的优化
opt-level = "s"         # 优化二进制大小, 不牺牲渲染性能 ("z"过于激进)
```

### 3.4 Step 3: Runtime Optimization (按需, 测量baseline后再决定)

| Optimization | Expected Benefit | Effort | Acceptance Criteria |
|-------------|-----------------|--------|-------------------|
| `Vec<PinnedRect>` 加容量上限 (e.g. 100) | 防止无限增长 | Low | 第101个pin时静默丢弃, 不panic, stderr提示 |
| DibCache 在 Idle 时释放缓冲区 | 减少常驻内存 | Medium | `DrawingState::Idle`持续10帧后调用`DibCache::destroy()`; 进入Drawing时自动重建 |
| 减少 `clone()` 调用 (状态机) | 减少峰值分配 | Medium | `process_event`中用`std::mem::take`替代部分clone |
| `mimalloc` allocator (可选) | 可能更紧凑 | Low (adds new crate dependency) | 仅当编译优化+运行时优化后仍超标时考虑; 会新增一个crate依赖 |

### 3.5 Step 4: Verification

- 用`--mem-report`重新测量 Working Set
- 与baseline对比, 量化优化效果
- 确认是否达到 <3MB 目标
- 如仍超标, 继续分析

### 3.6 Testing Strategy

- 编译优化: 运行全部现有测试确认无回归
- `--mem-report` flag: 启动后打印 Working Set + Pagefile Usage, 可CI自动化
- 运行时优化: 每项优化前后各跑一次`--mem-report`, 对比数字
- 结构体大小: `std::mem::size_of::<AppState>()` 和 `std::mem::size_of::<App>()` 断言不超过阈值

---

## 4. Implementation Order

1. **内存优化 — baseline测量** (实现`--mem-report`, 记录当前数字, 这是所有优化的基准线)
2. **内存优化 — 编译参数** (Cargo.toml优化, 再测一次, 对比baseline)
3. **热加载 — hook.rs** (RwLock改造 + update函数)
4. **热加载 — config.rs** (watch_config_dir + PartialEq)
5. **热加载 — overlay.rs** (App持有config, about_to_wait poll, PopupManager cascade)
6. **热加载 — main.rs** (启动watcher线程, channel连接)
7. **热加载 — 测试** (TDD: 先写测试再实现)
8. **内存优化 — 运行时** (按baseline和编译后数字决定是否需要)
9. **最终验证** (全部测试 + 内存测量对比baseline)

---

## 5. Non-goals

- 配置UI (托盘菜单编辑配置) — 不在此迭代
- 配置文件自动生成/模板 — 用户手写即可
- 跨平台文件监听 — v0.3再做, 现在用Win32 API
- 内存 <1MB — 过度优化, 3MB足够
