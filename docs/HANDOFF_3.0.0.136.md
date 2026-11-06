# 3.0.0.136 交接清单(2026-11-06)

均未提交。Desktop 分支 `3.0.0.136`;omp 分支 `kernel.2.0.10-universal`。

## 已完成(单元测试通过)

- 目录整理:配置 `~/.local/share/npi/desktop`、缓存 `~/.cache/npi`(含内核缓存、Rewind 快照)、worktree `~/Documents/NPi/Code/.worktrees` 与 `.agent-worktrees`、界面设置写到 `ui-state.json`;首次启动自动迁移(`npi_dirs.rs`、内核 `dirs.ts`)。
- 记忆统一:内核 `memory.backend` 在 Desktop 固定关闭;autolearn 只在 Code/Academic(会话覆盖文件 `session_policy`)。
- Memory 扩展:跨会话搜索 `search_conversations`(`session_search.rs`)、三份画像 `USER-CHAT/CODE/ACADEMIC.md`、用量透明、写入提示与撤销、技能可删(移到 skill-trash);修复设置页看不到 Agent 技能。
- 模型排班表:内核新增 flagship/high/standard/economy/arbiter 角色,smol/task/slow 未设时跟随对应档;Auto 子 Agent 模式下整批同档直接派发;设置 → Models 加"模型排班表"。
- 多专家评审 `expert_review`(仅 Academic):四档专家并行 + 仲裁,确认卡片后才运行,结果存 `research.db`(kind=review),MCP 也提供。

## 本次收口结果

1. 已修复两个失败测试：worker 测试填满六个槽位后检查限流与回收；Copilot MCP 不再假设只有一个工具。相关 Rust 22 项通过。
2. 已处理交接涉及的新增 clippy：保留并行线程先启动再 join 的必要集合并说明原因，直接写入输出字符串，简化 thinking 分支，整理回调类型、可见性、时长和错误文档。DevStore nightly 未安装 clippy；使用已安装工具链的严格全仓检查仍有范围外诊断，未声称全仓通过。
3. 已补 `research-core.md` 的模拟评审架构、`settings.md` 的排班表与继承/Auto 派发规则、CHANGELOG 和 PROGRESS。
4. 完整内核及 app 已重建；`dist/3.0.0.136/NPi Desktop_3.0.0.136_aarch64.dmg` 已替换旧包。278876587 字节，SHA-256 `9f6d6b92d4be551de6dc9e4f138c1b496a206e721dccf99aac6ca1b26854d809`。稳定证书 `NPi Desktop Local`，`hdiutil verify` 与挂载后 `codesign --verify --deep --strict` 通过。bundle 源 app/dmg 已清理，保留脚本、图标和增量缓存。未安装、未提交推送。
5. 验证边界：会话搜索/画像/记忆/技能移除 Rust 20 项、内核排班相关 227 项、前端 6 项检查通过；浏览器运行真实编译组件，点击评审接受/拒绝和技能 Undo，观察到正确回调。重建内核在临时配置下完成 CLI 排班配置读写、RPC `get_state`；随包 App 的真实 MCP 进程列出 `copilot_ask`、`expert_review`，未确认时不启动评审。没有发送真实模型请求。

## 仍待完成

- 原生 App 内的会话搜索、画像写入与提示、技能撤销，以及真实模型的排班派发、多专家评审端到端验收。需用户安装完整新包或确定隔离 App 验收方式；真实评审仍须确认卡片，不得绕过。本次未替换正在运行的应用或访问真实登录数据。

## 备注

- DevStore 过期租约已改名为 `lease.json.stale-20261105`(rust 映像目录),未删;建议报 DevStore bug。
- 用户 `config.yml` 开着 `autolearn.autoContinue: true`,Academic 开启 autolearn 后可能多出整理技能的模型调用。
