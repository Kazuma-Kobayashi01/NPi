# CLI 改版与工具输出预算交接

更新日期：2026-11-03。供 Claude 接手当前未提交工作树使用。

## 仓库与范围

- Desktop 根仓库：分支 `3.0.0.52`，基线提交 `092ac7c`。
- `omp/` 为独立 Git 仓库：分支 `kernel.2.0.10-universal`，基线提交 `348d690c43`。根仓库忽略它，必须分别检查两边的 diff。
- 本轮未提交、未推送。工作区开始时就有 Claude 的未提交改动；不能将所有 diff 归于本轮，也不能整体覆盖或回退。
- 原有 Desktop 改动包括 `desktop/CHANGELOG.md`、`src/app-live.jsx`、`src/app/i18n/settings.js`、`src/app/settings-schema.js`、`tests/test-settings-schema.mjs`。本轮 CLI 工作没有重写这些文件。
- 内核原有系统提示词、设置分层移除、主题与居中列等工作也需保留。下面记录的是接手后的增量及整合状态。

## 用户明确要求

1. 完成 `docs/archive/PLAN_CLI.md` 的阶段 1、2，并让普通 CLI 的思考默认折叠。
2. 优化 token 消耗只改工具返回内容量。子 Agent 调度和展示类模型调用是用户的 Agent 特色，必须保留；没有降低默认思考级别或子 Agent 并发、递归、预算。
3. Rust 操作使用 DevStore `rust/current`；不替换已安装应用、不动真实登录与会话数据。打包只生成 app、dmg，dmg 归集到根目录 `dist/{version}/`。

## 已实现的 CLI 改动

- 默认 Rail 输入框：填充面板、上下留白，框内底行显示 Build / Plan / Goal、当前模型、服务商；框外提示使用真实绑定，首页左侧目录、右侧版本。
- 首页在较高终端中整体下移；进入对话后使用普通会话布局。兼容较窄终端，保留其他输入框形状。
- Slash 命令列表在输入框上方打开；命令、设置、会话、服务连接列表共享 `packages/tui/src/chrome/list-dialog.ts` 的标题、Esc、边框和操作栏。
- 设置、会话、服务连接通过原有 alternate-screen overlay 居中显示，保持正常会话滚动区。设置支持跨页搜索，简单值左右切换，Ctrl+左右切页；子菜单和搜索保持各自的 Esc 语义。
- 会话显示用户名及日期分组；新增 Ctrl+D 删除、Ctrl+R 重命名、Ctrl+A 切范围。Delete / F2 / Tab 仍可用，删除仍经过确认。
- 服务连接按 Popular / Services 分组并始终支持搜索；保留禁用服务商过滤、授权及凭据验证逻辑。选中行会移除嵌套前景色后填色，避免文字与背景同色。
- 普通 ANSI CLI 思考默认只显示 Thinking / Thought 摘要，Ctrl+T 展开/收起（仅当前会话）。保存的初始折叠程度由 Appearance → Display → Collapse（`display.collapse`：Off / Thinking / Thinking + Tools / All，默认 All）控制，已取代原 `expandThinkingBlocks`。流式更新、结束和重绘不会意外重新展开；原始推理内容仍在会话中。原生 TSP 路径保留既有行为。
- 居中绘制时移除用户消息的 OSC 133 shell prompt 标记，避免终端将消息当作 shell 提示符重排，造成背景错位；复用现有 `stripPromptZones`。
- 工具挂载详细通知 `tools.xdevNotices` 默认关闭；工具列表变化记 debug 日志，模型仍能收到能力变化，用户可显式开启通知。
- 修复 Interaction Policy 的 Execution 分组未注册；补齐旧鼠标测试桩的 `composer.contentMargin`，以及输入事件测试中取消后应进入空闲的模拟状态。

主要实现位置：`packages/tui/src/{prompt/composer.ts,components/editor.ts,components/composer/rail.ts,status-line/component.ts,chat/assistant-message.ts,overlays/}`，以及 `packages/coding-agent/src/modes/{interactive-mode.ts,controllers/input-controller.ts,controllers/selector-controller.ts}`。

## 工具输出默认预算

实现主要在 `omp/packages/coding-agent/src/tools/settings.ts`，继续复用原有 artifact、截断、分页机制。

| 设置 | 原默认 | 新默认 |
| --- | --- | --- |
| `tools.artifactSpillThreshold` | 50 KiB | 12 KiB |
| `tools.artifactHeadBytes` | 20 KiB | 2 KiB |
| `tools.artifactTailBytes` | 20 KiB | 6 KiB |
| `tools.artifactTailLines` | 500 | 100 |
| `read.defaultLimit` | 300 行 | 120 行 |

这是默认值变更，已有显式配置仍优先，不会自动覆写用户配置。普通大结果保留首尾摘要和 artifact 回查；streaming OutputSink 沿用自身阈值、notice slack 与最终保护，因此不能声称所有结果精确为 8 KiB。字节数不等于 token 数，尚未测量真实会话整体节省比例。

文件范围读取仍提供续读位置；显式范围和配置的大页继续可用，受已有安全上限约束。artifact 原有 16 MiB 存储上限与超限策略未变。完整实现说明见 `omp/docs/tool-output-budgets.md`。

## 验证与边界

- CLI 界面：定向验证过中文消息边界、80/120 列与居中布局、菜单展开/关闭、设置搜索及左右切换、会话删除确认/取消后的布局、鼠标与滚动。
- 新回归 `packages/tui/test/cli-plan-ui.test.ts` 覆盖思考折叠/展开/流式结束、长标题及中文的弹窗宽度、模式变更后输入框身份行更新。
- 两包 TypeScript 检查、所改源码的 oxlint / oxfmt 检查通过；原生 transcript 回归也通过。
- 工具输出：167 项定向测试通过（`tools.test.ts`、`tools/read-artifact-in-memory.test.ts`、`read-leading-context-budget.test.ts`、`read-column-truncation-snapshot.test.ts`）。新增回归验证中等输出提前转存、artifact 中间内容回查、显式预算优先、默认 120 行与续读/范围/配置覆盖。
- 使用隔离数据检查了首页、命令、设置、服务、会话和折叠思考的源码渲染快照。没有用真实凭据调用模型或完整走登录授权流程，也没有宣称所有真实终端均已验收。
- Claude 原全量 coding-agent 日志为 20055 pass、378 skip、189 fail、1 error。它不是本轮改版新增缺陷数量；其中存在旧命令名/旧数据路径断言、原生功能与环境问题。本轮没有重新跑全量或逐项清完这些失败。

## 交付与后续

- CLI 最新开发内核：`omp/packages/coding-agent/dist/npi`（已包含 CLI 改版和较小工具输出预算）。版本仍为 `npi/2.0.10`，不等同于 Desktop 版本。
- Desktop 当前配置：package 版本 `3.0.0.52`，Tauri/Cargo 版本 `3.0.0-52`；这是原有命名映射，本轮打包不另行升级版本。
- dmg 构建前需要把最新开发内核原子替换到 `desktop/src-tauri/binaries/npi-aarch64-apple-darwin`，再用 DevStore 构建 app,dmg。不会覆盖 `/Applications`。
- 本次 app/dmg 已构建并校验，产物与包装限制见下方。
- 剩余：阶段 3 的用户消息竖线、回复尾行、右侧 Context 栏、会话标签条取舍，以及阶段 4 的可选细节。真实终端和真实账号的最终体验仍需用户确认。
- 后续 token 优化须继续遵守用户范围：不削弱子 Agent 与展示能力；优先观察实际工具输出用量及用户显式预算配置。

### 本次 app/dmg 构建

- 构建日期：2026-11-03。Desktop `3.0.0.52`，macOS Apple Silicon（aarch64）；内核 `npi/2.0.10`。
- 已把开发内核原子替换到构建侧 `desktop/src-tauri/binaries/npi-aarch64-apple-darwin`。替换前后的开发内核 SHA-256 一致：`99e52698895502d75eb7fd89aba9a2ad4167899ce82994822ca3f520c2815895`（app 签名会重写随包二进制签名）。
- 通过 DevStore `rust/current` 运行 Tauri release 构建，前端编译 113 个 JSX 文件，生成 `desktop/src-tauri/target/release/bundle/macos/NPi Desktop.app`。
- Tauri 的 dmg 外观 AppleScript 被 Finder 拒绝（`A privilege violation occurred. (-10004)`）。随后用本次生成的 `bundle_dmg.sh --skip-jenkins` 完成镜像包装，保留 Applications 拖放链接；未设置 Finder 自定义背景和图标位置。未修改系统自动化权限或源码打包配置。
- 正式归集产物：`dist/3.0.0.52/NPi Desktop_3.0.0-52_aarch64.dmg`，274485116 字节。
- dmg SHA-256：`82a4b70b77842b490a3c0ebc410393586e5e4debb5ba48b832b732f4fafab116`。
- `hdiutil verify` 通过；只读挂载后确认包内版本 `3.0.0-52`、Applications 链接正确，`codesign --verify --deep --strict` 通过，已卸载检查卷。
- 使用现有 ad-hoc 签名，未公证；没有替换已安装应用，没有改动用户凭据或会话。仅生成 app 和 dmg，没有生成 zip。
- Desktop 定向回归：settings-schema 15 项、i18n 11 项通过；Python 离线运行时资源完整性检查通过。

