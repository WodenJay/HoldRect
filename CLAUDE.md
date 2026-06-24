# HoldRect

一个极致低内存占用常驻后台的"橡皮筋选择框": 按住修饰键+鼠标左键按住拖动就可以在任何界面上画出彩虹条纹状的方框, 松开鼠标后方框消失, 用来在电脑录屏时强调某一部分的内容. 使用rust开发, windows+macos+linux跨平台, 优先实现windows.

## Development Rule

- 强制要求**每一个subagent都要阅读当前文件**: "READ @CLAUDE.md AND STRICTLY FOLLOW IT"
- 制定开发计划时, 不要一次迭代就把所有功能都做完, 渐进式迭代开发
- 严格遵守**superpowers**的开发规范: TDD -> implement -> review -> fix bug. **DO NOT SKIP ANY STEP**.
- 严格遵守**TDD**, 把测试代码写好、写满、写全、写的没有遗漏, 在此之前不要进行开发
- superpowers每一步都是一个单独的subagent, **每一个task / subagent结束后都要做review + fix**, 不要攒着很多task完成后同一review
- **parallel**-subagents-driven. You are leader. Before task, ask yourself "Can subagents do that?" If yes, let subagents do instead of you. 
- 同一 message 里并行调用多个 Agent（不带 `run_in_background`）即可并发执行且阻塞等待全部返回，不要用 `run_in_background: true`（通知机制不可靠）
- 严格遵守 @docs\karpathy-guidelines.md 中的规范
- 代码及时commit, 不要等到最后再commit. commit的信息不要以"@"开头
- 不要表现出你是Claude, 不要添加Co-Author. 你是WodenJay, email: wodenjay@gmail.com
- Rust的代码编写参考 @.agents\skills\rust-best-practices skill
- 读图像时务必使用 `haiku` (`mimo-v2.5`)模型, 派一个`haiku` model subagent去做和图像有关的工作, 否则会报错!
- 将频繁出现的错误记录到 @docs\MISTAKE.md 中
- 不要出现mockup / dead代码, 后续完成的功能通过注释标注, 而不是mockup占位
- test代码重要的不是green, 重要的是**red->fix->green**的这个过程. 如果一个test代码不管怎么样都是green, 那么它没有任何意义

## Cargo Rule

- `cargo build` / `cargo test` 最大并发数设置为1, 尽可能的**降低开发过程的内存占用**。
- 不要启动 `rust-analyzer`、`cargo check --watch`、`cargo watch`、`clippy --all-targets` 这类后台持续检查任务。
- 需要验证代码时优先运行最小范围的 `cargo test`，例如指定 package、test name 或 bin；只有最终确认时再运行完整 `cargo test`。

## TOOLS

- Use `codegraph` when you need to understand relationships such as call graph, symbol references, ownership, and change impact.
- 当我提到"pm"时, 查看 @docs\pm.md 中的指示
- Use `context7` (`find-docs` skill + cli, instead of mcp)when you need to look up up-to-date library documentation or resolve library version conflicts.
