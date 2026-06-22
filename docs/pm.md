# PM Plugin Usage Rules for FlashNote

Use these rules when working on FlashNote product direction, packaging, marketing, growth, launch, GitHub stars, downloads, README, positioning, feature prioritization, user research, or competitive analysis.

Do not use these PM plugins for coding, debugging, refactoring, architecture implementation, dependency fixes, build errors, tests, or performance engineering. Development work is handled separately.

## FlashNote context

FlashNote is a lightweight note-taking app focused on instant capture. The core user need is: press a global hotkey, open a small note window immediately, and record an idea before it disappears.

Primary product direction:

- Instant capture within about 0.5 seconds.
- Very low background memory usage.
- Local-first, simple, reliable note storage.
- Minimal UI, no heavy knowledge-base workflow by default.
- Built for people who need to capture thoughts, ideas, tasks, code snippets, research notes, and brainstorming moments quickly.
- GitHub success target: more stars, clearer positioning, stronger README, more attractive demo, and features that make users want to download and recommend it.

## General decision rule

When a request is about product, growth, positioning, market, launch, or feature choice, choose one primary PM plugin first. Use a second plugin only if the request clearly spans two stages.

Always give a concrete recommendation. Avoid vague tradeoff-only answers. Prefer: "choose A", "ship B first", "delete C", "position it as D".

Do not produce long PM theory. Apply the framework and return the decision, reasoning, and next action.

## Plugin selection

### 1. pm-product-strategy

Use when the task is about product identity, long-term direction, positioning, value proposition, differentiation, vision, pricing, monetization, or business model.

Typical FlashNote triggers:

- "FlashNote 应该怎么定位"
- "一句话介绍 FlashNote"
- "这个产品的核心卖点是什么"
- "和 Notion / Obsidian / Apple Notes / Joplin / Raycast Notes 有什么区别"
- "README 顶部应该怎么写"
- "FlashNote 应该强调极快、轻量、本地优先还是 AI"
- "要不要做付费版 / Pro 版"
- "产品路线应该聚焦哪里"

Useful commands or skills:

- `/strategy`
- `/value-proposition`
- `/business-model`
- `/pricing`
- `product-strategy`
- `value-proposition`
- `product-vision`
- `monetization-strategy`

Default output for FlashNote strategy tasks:

1. Final positioning.
2. Target user.
3. Core value proposition.
4. Differentiation from existing note apps.
5. What to emphasize in README and screenshots.

### 2. pm-product-discovery

Use when the task is about deciding what feature to build, what to remove, what to ship first, which assumption is risky, which user problem matters most, or how to validate product demand.

Typical FlashNote triggers:

- "FlashNote 还应该加什么功能"
- "哪个功能最容易拿 star"
- "MVP 应该包含什么"
- "这个功能要不要做"
- "提醒、Markdown、图片、搜索、同步、AI 总结、标签、托盘、快捷键，先做哪个"
- "用户真正需要什么"
- "怎么验证大家会不会用"
- "帮我做功能优先级"

Useful commands or skills:

- `/discover`
- `/brainstorm`
- `/triage-requests`
- `/interview`
- `/setup-metrics`
- `prioritize-features`
- `identify-assumptions-new`
- `brainstorm-experiments-new`
- `opportunity-solution-tree`

Default output for FlashNote discovery tasks:

1. Feature decision: build / delay / reject.
2. Priority order.
3. Why this feature helps stars or downloads.
4. Smallest shippable version.
5. Validation method.

### 3. pm-market-research

Use when the task is about users, competitors, market segments, customer journey, GitHub audience, or finding where FlashNote can win.

Typical FlashNote triggers:

- "帮我分析竞品"
- "FlashNote 和 Obsidian / Notion / Joplin / Simplenote / Apple Notes / Raycast / Flow Launcher / Tot / Drafts 比有什么机会"
- "目标用户是谁"
- "哪些人最可能给 star"
- "developer 用户和普通用户应该优先做谁"
- "GitHub 上类似项目有什么卖点"
- "这个市场是不是已经太卷"
- "用户会因为什么从其他笔记软件切过来"

Useful commands or skills:

- `/research-users`
- `/competitive-analysis`
- `/analyze-feedback`
- `user-personas`
- `market-segments`
- `customer-journey-map`
- `competitor-analysis`

Default output for FlashNote market research tasks:

1. Best target segment.
2. Competitor gap.
3. User pain point.
4. FlashNote winning angle.
5. README or feature implication.

### 4. pm-marketing-growth

Use when the task is about GitHub stars, README packaging, product naming, tagline, launch copy, screenshots, demo GIF, growth loop, social post, landing page copy, or making FlashNote look attractive.

Typical FlashNote triggers:

- "怎么让 GitHub star 更多"
- "README 怎么写更吸引人"
- "帮我写 tagline"
- "FlashNote 的 slogan"
- "怎么包装这个功能"
- "首页截图应该展示什么"
- "demo GIF 应该怎么拍"
- "GitHub repo 描述怎么写"
- "Product Hunt / Hacker News / Reddit 发布文案"
- "怎么让用户愿意下载"
- "这个功能怎么宣传"

Useful commands or skills:

- `/market-product`
- `/north-star`
- `marketing-ideas`
- `positioning-ideas`
- `value-prop-statements`
- `product-name`
- `north-star-metric`

Default output for FlashNote marketing-growth tasks:

1. Final marketing angle.
2. README headline or repo description.
3. 3 to 5 short selling points.
4. Screenshot or demo GIF plan.
5. Distribution idea for GitHub stars.

### 5. pm-go-to-market

Use when the task is about launch strategy, release timing, target launch audience, channels, Product Hunt, Hacker News, Reddit, X/Twitter, developer communities, first users, or growth loops after release.

Typical FlashNote triggers:

- "FlashNote 怎么发布"
- "在哪些平台宣传"
- "Product Hunt 怎么发"
- "Hacker News 怎么发"
- "Reddit 发哪里"
- "第一批用户怎么找"
- "怎么设计 launch checklist"
- "怎么让用户持续推荐"
- "正式开源前应该准备什么"
- "发版顺序怎么安排"

Useful commands or skills:

- `/plan-launch`
- `/growth-strategy`
- `/battlecard`
- `gtm-strategy`
- `beachhead-segment`
- `ideal-customer-profile`
- `growth-loops`
- `gtm-motions`

Default output for FlashNote go-to-market tasks:

1. Beachhead audience.
2. Launch channel priority.
3. Launch message.
4. Pre-launch checklist.
5. First-week growth actions.

## Priority map

Use this priority map when the request is ambiguous:

- Deciding what FlashNote is: use `pm-product-strategy`.
- Deciding what FlashNote should build next: use `pm-product-discovery`.
- Comparing FlashNote with other tools: use `pm-market-research`.
- Making FlashNote look attractive on GitHub: use `pm-marketing-growth`.
- Planning public release and promotion: use `pm-go-to-market`.

## Common combined workflows

### Feature that can increase GitHub stars

Use:

1. `pm-product-discovery` to rank feature ideas.
2. `pm-marketing-growth` to package the chosen feature into a README/demo-worthy selling point.

Return:

- One feature to build first.
- One feature to delay.
- One feature to reject.
- README wording for the chosen feature.

### README or GitHub repo rewrite

Use:

1. `pm-product-strategy` to clarify positioning.
2. `pm-marketing-growth` to write headline, selling points, and demo structure.

Return:

- Repo one-liner.
- README hero section.
- Feature bullets.
- Demo GIF plan.
- Star-oriented call to action.

### Launch plan

Use:

1. `pm-market-research` to identify the best first audience.
2. `pm-go-to-market` to create launch sequence.
3. `pm-marketing-growth` to create launch copy.

Return:

- Target audience.
- Launch channels.
- Launch order.
- Copy for each channel.
- First-week checklist.

### Competitor-based differentiation

Use:

1. `pm-market-research` for competitor analysis.
2. `pm-product-strategy` for differentiation and positioning.
3. `pm-marketing-growth` for README wording.

Return:

- Competitor table.
- FlashNote's strongest gap.
- Positioning sentence.
- Feature or README implication.

## Output style

For FlashNote PM tasks, use direct conclusions.

Prefer this structure:

```markdown
Conclusion: ...

Use: <plugin name>

Decision:
- ...

Reason:
- ...

Next action:
- ...
```

Avoid this:

- Long generic explanations.
- Pure framework descriptions.
- Saying every option depends on context.
- Suggesting features without ranking them.
- Mixing development implementation details into product strategy.

## FlashNote default product bet

Unless the user says otherwise, assume the best product bet is:

FlashNote should win by being the fastest, lightest, most frictionless local-first note capture tool, rather than becoming a full knowledge-base app.

Therefore:

- Prefer features that reinforce speed, low memory, instant capture, and trust.
- Be cautious with heavy AI features, complex databases, collaboration, cloud sync, and large workspace systems.
- README should make the user understand the product in 5 seconds.
- Demo should show hotkey → instant popup → type → save → search/history.
- GitHub star strategy should emphasize clear niche, polished demo, measurable speed, low memory usage, and developer-friendly local-first design.
