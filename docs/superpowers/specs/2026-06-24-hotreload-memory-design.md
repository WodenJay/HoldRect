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

使用Win32 `ReadDirectoryChangesW` 监听 `~/.holdrect/` 目录:

- 如果 `~/.holdrect/` 目录不存在, watcher线程静默退出(跟当前config加载行为一致)
- 如果 `~/.holdrect/` 存在但 `config.toml` 不存在, 监听目录等待文件创建
- 编辑器保存文件可能触发多次事件(rename + modify), 用简单去抖: 收到事件后sleep 100ms再读取

```rust
fn watch_config_dir(dir: PathBuf, tx: Sender<AppConfig>) {
    // 0. 如果目录不存在, 静默退出
    // 1. CreateFileW 打开目录, FILE_LIST_DIRECTORY access, FILE_FLAG_BACKUP_SEMANTICS
    // 2. 分配 FILE_NOTIFY_INFORMATION buffer
    // 3. Loop:
    //    - ReadDirectoryChangesW (overlapped, 阻塞等待)
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
| `modifier_name` | ✅ | Used in popup display |

### 2.7 Error Handling

- Config file deleted → keep current config, stderr warning
- Config file has syntax error → keep current config, stderr warning with error detail
- Config file has partial valid fields → use parsed fields, missing fields keep current values (current behavior via `AppConfig::parse` defaults)

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

### 3.2 Step 1: Measurement

- 编译release版本, 启动后用Process Explorer查看:
  - Working Set (物理内存)
  - Private Bytes (提交内存)
- 记录baseline

### 3.3 Step 2: Compile-time Optimization

```toml
# Cargo.toml
[profile.release]
lto = true              # 链接时优化, 减少死代码
strip = true            # 去掉符号表和调试信息
panic = "abort"         # 不展开栈, 减少panic infrastructure
codegen-units = 1       # 更好的优化
opt-level = "z"         # 优化大小
```

### 3.4 Step 3: Runtime Optimization (按需)

| Optimization | Expected Benefit | Effort |
|-------------|-----------------|--------|
| `Vec<PinnedRect>` 加容量上限 (e.g. 16) | 防止无限增长 | Low |
| DibCache 在 Idle 时释放缓冲区 | 减少常驻内存 | Medium |
| 减少 `clone()` 调用 (状态机) | 减少峰值分配 | Medium |
| 考虑 `mimalloc` allocator | 可能更紧凑 | Low (adds dep) |

### 3.5 Step 4: Verification

- 重新测量 Working Set
- 确认是否达到 <3MB 目标
- 如仍超标, 继续分析

### 3.6 Testing Strategy

- 编译优化: 运行全部现有测试确认无回归
- 运行时优化: 测试不变, 关注内存指标

---

## 4. Implementation Order

1. **内存优化 — 编译参数** (零代码改动, 先拿免费收益)
2. **内存优化 — 测量** (确定baseline和编译后数字)
3. **热加载 — hook.rs** (RwLock改造 + update函数)
4. **热加载 — config.rs** (watch_config_dir + PartialEq)
5. **热加载 — overlay.rs** (App持有config, about_to_wait poll)
6. **热加载 — main.rs** (启动watcher线程, channel连接)
7. **热加载 — 测试** (TDD: 先写测试再实现)
8. **内存优化 — 运行时** (按测量结果决定)
9. **最终验证** (全部测试 + 内存测量)

---

## 5. Non-goals

- 配置UI (托盘菜单编辑配置) — 不在此迭代
- 配置文件自动生成/模板 — 用户手写即可
- 跨平台文件监听 — v0.3再做, 现在用Win32 API
- 内存 <1MB — 过度优化, 3MB足够
