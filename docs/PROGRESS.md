# 总进度与下一步

整理于 2026-11-14(分支 `3.1.0.103`)。这是**唯一的进度入口**:`PLAN.md`(1.1.0 规划,唯一仍在推进的计划)写"打算做什么";已完成的计划与交接记录在 [`archive/`](archive/README.md),本文写"现在到了哪里、下一步做什么"。每做完一批就回来改这里。

验证记录:用户装 `3.0.0.132` 后在真实 app 里点过上面列的功能(含 3.0.0.122–132 新增部分),反馈没有问题;2026-11-06 用户装 `3.0.0.136` 完整包完成验收(会话搜索、画像、技能撤销、模型排班、多专家评审)。逐行的"未验证"标记保留原样,没有逐条改,以后遇到具体问题再回写。

状态词:**已做**=代码和单元测试完成;**已验证**=在真实 app / 真实数据里确认过;**未验证**=只有编译与单元测试。

## 内核分支与上游移植（2026-11-15，已提交、未推送、未打包）

- `kernel.2.1.1-universal`（`836bccb6c3`）：把 `kernel.2.1.0-universal` 工作区的全部改动提交为下一个版本（Remote Connect、`/rc`、按块审批、Generative UI、Hydrogen、用量减负、会话名乱码修复等，106 个文件）。
- `kernel.2.1.1-coldfix`，在其上移植上游 oh-my-pi（我们基于上游 `6a86df8539`，与上游无共同 git 历史，只能逐个摘取）：
  - `fcc6ecd90e`：暂停 Google Antigravity。移植上游 `55689486f8`（KDL `disabled` 规则）、`e3fde96e2a`（`login-warning`）、`9a37046d3a`（暂停）。上游理由：Google 会封停在第三方工具里登录 Antigravity 的账号（上游 #18341）。规则在时，新登录、已存凭据、用量查询和请求都不会到达 Antigravity；删掉 `disabled` 一行即恢复。
  - `0c49301831`：Gemini 4 长回答跨 65,536 token 分段续写（上游 `74e3cfa574`，#19081），Gemini API 与 Vertex 的输出上限改为 262,144。`models.json` 保留我们自己的（上游表无法单独合并），`rules.json` 重新生成。
  - 未移植：上游其余 Hydrogen 提交（多为 Antigravity 订阅档位折叠，已暂停用不上）、第二档会话与账号修复。
- 验证：三个包类型检查通过；ai 包全量 6800 项中 11 项失败，其中 7 项在移植前的提交上同样失败，4 项为全量负载下的 5 秒超时、单独运行通过；内核远程、`/rc`、RPC、模型发现等 250 项中 1 项失败，同样是原有问题。

## 用量查询减负（2026-11-15，内核已随 `kernel.2.1.1-universal` 提交，Desktop 未提交，未打包）

- 起因：一个 Google 账号的 Antigravity 服务被 Google 以违反服务条款停用（日志 `403 PERMISSION_DENIED … violation of Terms of Service`，首次 11-14 11:00）。根本原因是内核冒充 Antigravity 官方客户端（上游设计）；NPi 额外放大了请求：桌面端每轮对话结束都 `npi usage invalidate` + `npi usage --json`，强制向所有已登录提供方的额度接口重新查询。
- 桌面端：一轮结束后只读内核缓存，且同一 profile 每分钟最多读一次；只有手动刷新才强制重新查询（`app/use-quotas.jsx`，`session-model.md` 已更新）。
- 内核（所有提供方）：用量查询遇到 401/403 后冷却 6 小时，手动失效也不绕过；提供方遇到 403 时即使不抛错、只返回空结果也同样处理。Antigravity 在额度接口被拒后不再继续查模型列表和 sandbox 接口。
- 已验证：ai 包用量/认证相关 830 项测试通过（含新增 3 项），类型检查通过；Desktop `npm test` 通过。
- 待定：后台任务（Memory、标题生成、维护路由）是否继续用 Antigravity（方案 A/B，用户未定）。

## 内核 `/rc` 远程控制与会话名乱码（2026-11-15，未提交、未重编译）

- 内核 `kernel.2.1.0-universal` 工作区新增 `/rc`：已配对的手机直接接入终端正在运行的会话（不另起进程）。手机发的提示词在这个会话里执行并显示在终端；审批和 ask 弹窗两边同时出现，先答的生效，另一边自动撤回。用法 `/rc <wss://relay>`（保存到 `remote.relayUrl`）、`/rc --listen host:port`、`/rc status`、`/rc off`。手机端不能新建、切换、分支、fork 或跳转终端的会话。需要先用 `npi remote pair` 配对。
- 实现：`runRpcMode` 新增嵌入模式（输出走内存管道，不接管终端弹窗、不初始化扩展、不调度 goal、不写 stderr，断开时只停止监听不销毁会话）；守护进程新增“终端会话通道”（不回收、手机关不掉）；弹窗抢答从 collab 访客扩展为多方（`guest-ui-peers.ts`）。
- 会话名乱码已修：列表只读前 4 KB，长中文/emoji 首条消息被截在字符中间，显示为 `�`。
- 已验证：新增 `remote-control.test.ts`（嵌入服务 + 守护进程 + 模拟手机走加密通道：会话匹配、弹窗应答与撤回、断开后会话仍在）和乱码回归测试；相关远程/RPC/弹窗测试 78 项通过，类型检查通过。`test/rpc` 剩 1 项 goal 测试失败，改动前同样失败；collab 注册表冒烟 1 项失败，原因是注册目录 `.omp` 与 NPi 不一致，与本次无关。
- 追加（2026-11-15，未重新打包）：设置 `remote.autoStart`（Interaction → Remote Control → Start /rc Automatically，默认关）在启动交互会话时自动 `/rc`；`/rc` 模式下手机只能看到和打开终端正在共享的那个会话（列表过滤、其他会话和新建会话被拒，`host.info` 带 `remoteControl: true`）。手机端（`npi-remote`，未编译：Flutter/Android 环境所在外接存储离线）已写：采用守护进程返回的通道 id、`/rc` 模式自动打开该会话并隐藏“新会话”、会话离线时显示 Claude 式的离线卡片与“已与电脑断开”提示条并禁用输入。
- 已打包：`dist/3.1.0.103/NPi Desktop_3.1.0.103_rc_aarch64.dmg`（见 RELEASES）。未验证：没有在真实终端和手机上跑过 `/rc`。

## 对话内交互 P0–P2（2026-11-13，未提交、未打包）

- 工作区：Desktop `3.1.0.102`，内核 `kernel.2.0.10-universal`；与 Hydrogen 改动同在未提交工作区。
- Generative UI（内核 + Desktop 镜像）：`ui.present` 增加 `ui_id` 参数，可原地更新 display 卡片（持久化为 `ui.update`，按 revision 重放）；新增 `suggestions`（chips 用于追问，cards 用于非阻塞选择，点击后作为普通 prompt 发出，运行中作为 follow-up）、`ranking`、`path-picker`（提交时内核校验路径存在、类型匹配且在项目内）、schema 级 `steps` 向导；Display 表格支持排序、筛选、复制 CSV。RPC wire 已重新生成（`revision`、`hunks`、`selectedHunks`）。
- 写入审批可按 hunk 部分接受（`approval-hunks.ts`）：只写入勾选的 hunk，文件已变化则拒绝执行；结果会告诉模型哪些 hunk 被拒；全选等同 Approve，全不选等同 Deny。部分接受不会生成长期授权。
- Desktop：选中回复文本后出现 Quote / Ask / Steer 浮条；回复 👍/👎，原因只存本地，选择 Tell assistant 时作为普通消息发出；对话内计划支持勾选、重排、删除，暂存后 Apply，运行中走 steer，空闲走 prompt。
- 已验证：内核 generative-ui / approval-hunks / rpc-wire 定向测试 41 项通过，`tsc --noEmit` 干净，`sdk/rust/omp-rpc` 的 `cargo check`（DevStore）通过；Desktop `npm test` 全部通过（含 dist 构建）。用 Chromium 加载 dist 组件做了冒烟（桥接为桩）：原地更新、排序与筛选、suggestion 发送、向导 ranking + path-picker 提交、部分 hunk 应答、计划 Apply、反馈持久化、选区 Quote 均符合预期。
- 未验证：真实 Tauri 应用和真实模型调用；tools/rpc 目录下另有 49 个失败用例，集中在 browser relay、Tern、computer、read 截断、IRC、Fireworks，与本次改动的文件无关，未做基线对比。
- 遗留已修（同日）：
  - 计划块会从历史重建（`adapter.js` 的 `buildInlinePlan`，按 `toolCallId` 挂到发起调用的那一轮），切换标签或重开会话后不再丢失。
  - 修了一个连带 bug：内核的工具名是 `todo` 而不是 `todo_write`，原先实时事件从未触发计划块和看板更新；计划批准提示词和计划编辑消息也已改为 `todo`。
  - Quick Ask 窗口接入了选区浮条（Quote 写入窗口输入框）、👍/👎，以及 suggestion / path-picker 回调。
  - Generative UI 卡片和计划编辑的固定文案已接入四语言（`gui.*` / `plan.*`）。
  - 验证：`npm test` 全部通过，`test-transcript-merge` 新增历史计划块用例；在 Chromium 中以 zh-CN 渲染 QuickWindow（桩桥接）冒烟通过，确认历史计划块可编辑、Quote 写入输入框、suggestion 能发出。
- 仍未处理：计划块标题栏的“打开看板 →”按钮没有点击处理（原有问题）。

## NPi for VScode（2026-11-08）

- 根目录 `npi-vscode/`，独立 Code 侧栏，不包含 Chat／Academic，不改 Desktop 或内核。插件当前版本 `0.1.1`，macOS Apple Silicon VSIX 内置独立 NPi 后端，默认不再依赖全局 CLI 或 PATH；显式 `npi.executablePath` 仍可覆盖。
- 已实现流式 Markdown／思考、工具卡与输出、原生审批／提问、模型与推理选择、编辑器文件／选区附件、独立会话持久化、新建／切换／重连。
- 已在 macOS arm64、VS Code `1.141.0`、真实 `npi/2.0.10` 开发宿主中验证侧栏发送、选区附件、实际工具写入、原生 write 审批、候选模型／推理发现、新会话、历史切换、重连恢复；使用临时 HOME 和本地确定性模型端点，未读取真实凭据或调用付费云模型。传输边界回归 3 项通过。
- 当前包 `npi-vscode/npi-vscode-0.1.1-darwin-arm64.vsix`（约 112.7 MB，内置已签名的 `npi/2.0.10`）；未安装到用户日常 VS Code、未发布 Marketplace。旧 `0.1.0` 包仅为历史产物。使用与权限边界见 `npi-vscode/README.md`。
- 界面与插件 README／Changelog 已改为英文，含按钮、提示、可访问性标签、思考／工具状态与 `lang="en"`；不更改用户内容或模型回复语言。真实 VS Code Webview 已验证空态与临时展示夹具的运行／完成／失败状态，重新生成同路径英文 VSIX，无模型请求。
- 输入区参照 Copilot 重排：附件胶囊、+ 上下文菜单、模型／推理与右侧发送／停止均收进卡片；卡片下方显示 Local、启动审批策略与日志／重连。真实 Webview 验证 260／320／480 px 下空闲和运行态无横向溢出、发送／停止不被挤出，原生附件菜单可选代码，移除附件不发送草稿；按钮事件通过临时宿主拦截验证，未发送模型请求。
- `0.1.1` 后端直接沿用标准 `~/.local/share/npi/agent` 配置、订阅认证、画像/profile 与模型排班，不复制凭据到插件目录；侧栏会话仍独立。新增内核命令菜单、原生订阅登录入口，以及随包 NPi 完整 Terminal（终端专用设置与面板不在 Webview 重造）。
- 修复并复现前后验证：命令输出被后续 snapshot 清空、运行中工具输出在侧栏重建后丢失。定向回归共 5 项通过。
- 真实 VS Code `1.141.0` 隔离开发宿主验证：PATH 无 `npi` 仍启动随包后端；仅在临时 HOME 的标准系统 NPi 目录放模型配置与测试认证，实际请求携带预期测试 bearer key；侧栏请求、真实文件写入、原生 exec 审批、命令发现与输出保留、重连历史、订阅提供商菜单（取消，未登录）、完整 NPi Terminal 均已跑通。未读取真实凭据、未完成真实 OAuth 或请求付费云模型，未覆盖其他平台与远程宿主。
- 最终 VSIX 已安装到临时 VS Code 用户/扩展目录（非日常配置）并启动验证：从安装后的 runtime 执行，加载同一临时系统配置完成请求并归于 Ready；解包内核可执行权限、严格签名校验通过。VSIX SHA-256：`19f890adae6c4765e1d54171e327894c7a091ada90392e3b2d03279524f75d3e`。临时宿主与本地端点已关闭。

## 0. 先看这三件事

1. **已提交并推送(2026-11-05)。** Desktop 分支 `3.0.0.121` 已提交(`bb1d949` 功能、`5f36015` 文档、`36053d3` 版本号)并推送到 `origin/3.0.0.121`;`3.0.0.101`(`bb79b6e`)同时推送。`3.0.0.102`、`3.0.0.103` 与 101 指向同一个提交,没有推送(没有独立内容)。omp 仓库 `kernel.2.0.10-universal` 提交 `52a22ce3e0` 并推送到 `NPi-Kernel`。
2. **两个仓库都已提交并推送**:Desktop 分支 `3.0.0.132`(含 3.0.0.122–132 的全部提交),`omp` 的 `kernel.2.0.10-universal`。 之后的改动从 `3.0.0.121` 继续,版本号要升时新建分支。
3. **平台范围:只做 macOS**(2026-11-04 决定);**Windows 适配交给别人**(2026-11-05)。`PLAN.md` 里"必须兼容 Windows"的要求在本仓库作废(见 §6),相关内容留作交接参考。

## 1. 版本与产物

完整版本表、打包流程、内核构建与签名证书说明已搬到 [`RELEASES.md`](RELEASES.md)。当前最新:Desktop `3.1.0.103`(Remote 包 `dist/3.1.0.103/NPi Desktop_3.1.0.103_remote_aarch64.dmg`,未提交、未安装),内核 `kernel.2.1.0-universal` 工作区。

## 2. 已做的工作(按领域)

### Desktop 基础与体验

| 项 | 状态 | 说明 |
|---|---|---|
| 快速询问(Option+Space) | 已做、**未验证** | 根因是导航防护拦了它的页面,已放行;自绘玻璃 + 流光 + 系统模糊底层;下拉菜单在条下方,窗口随之增高 |
| Cmd+W 收到菜单栏图标 | 已做、**未验证** | 根因是快捷键表把 Cmd+W 绑成了"关闭标签页";现在 Cmd+W 走 `hide_main_window`,关闭标签页只剩 Ctrl+W |
| 菜单栏图标与 Dock 切换 | 已做、**未验证** | 用项目 logo;窗口隐藏时 Dock 图标消失 |
| 二级页面独立窗口 | 已做、**未验证**(3.0.0.137) | Librarian、阅读器、Write、用量统计只调命令;历史会话、审批规则、Plan、Git 浏览器经 `app/panel-link.js` 跟主窗口通信(主窗口推快照,面板的调用回到主窗口执行,按种类白名单),见 `desktop/docs/architecture/panel-windows.md`。**设置不做独立窗口**(2026-11-06 决定):它要用主窗口几乎全部状态,留在主窗口内 |
| macOS 通知 | 已做、**已验证(用户确认授权可用)** | 根因是临时签名,每个版本被系统当成新应用(`ncprefs` 里 `dev.npi.desktop` 已有 9 条以上来源,授权值为 0)。现已用稳定证书签名,用户确认授权可用 |
| 图片 429 | 已做、**未验证** | `Retry-After`(上限 4 秒)、429/503 重试两次并递增等待、同域名请求间隔 250 毫秒。**没找到这次 429 的原文**:如果来自模型接口或搜索源,这次改动覆盖不到 |
| 左图右文图片卡片 | 已做、**未验证** | `ui.present` 图片组件新增 `description`;内核校验、桌面渲染、Chat 提示词都改了;需要内核 3.0.0.103 之后的版本 |
| 生图 (`generate_image` / GPT-Image-2) | 已做、**已验证**(Codex Plus 实测出图 + UI 灯箱冒烟，2026-11-08) | 全局配置保存 `generate_image.enabled: true`；实测 `openai-codex/gpt-image-2` 由 `gpt-image-2-codex` 生成 WebP；Desktop 工具卡接通 `generate_image` 与 `write xd://generate_image` 缩略图/灯箱/模型信息与错误态；Chat 按会话配置条件放行 `generate_image` |

### Chat

| 项 | 状态 | 说明 |
|---|---|---|
| 裁剪为只对话 | 已做、用户反馈"快多了" | 内置工具只留 `web_search`、`read`,另有展示/交互 UI;去掉编码工具、MCP、图书馆查询 |
| `remember` 工具 | 已做、**未验证** | 往记忆文件追加一行,有大小上限与去重 |
| 一键转 Code/Academic | 已做、**未验证** | 工具 `handoff_to_work` → 输入框上方的卡片 → 用户选文件夹 → 新开对应模式标签页,提示词填入输入框不发送 |
| 首字慢的分析 | 已做 | 结论:`google-antigravity` 通道非流式(首字≈总耗时,`stats.db` 实证);已缩短提示词、压缩阈值 70→85。未改思考档位(用户否决) |
| 首字耗时埋点 | **不做**(2026-11-06 决定) | 需要时可用 `~/.omp/stats.db` 的 `ttft` 对比;用量页已显示平均 ttft |

### Academic:Librarian(文献库)

| 项 | 状态 |
|---|---|
| L0 数据层、L1 窗口、L2 引用格式(hayagriva)、L3 PDF 导入与 Crossref、L4 Copilot 工具与引用核对、回收站、权限测试 | 已做、**未对真实 PDF / 真实服务 / 真实模型验证** |
| 窗口搜索接入 FTS5(中文短词) | 已做、未验证 |

### Academic:Research Core(`PLAN_3.0`)

| 项 | 状态 | 说明 |
|---|---|---|
| 库 schema v5/v6、页级索引、锚点、CJK FTS5、引文核对 | 已做 | v6 加摘要与对照表;旧库升级时保留备份 |
| 阅读器窗口 | 已做、**未验证** | 文献列表/章节/分页原文/搜索/锚点跳转;入口在 Academic 工具栏菜单"阅读器" |
| 摘要(L1/L2)与对照表 | 已做、**未验证** | 引文经 Rust 核对后标"一致/未找到/模型理解";`not_reported`、`not_comparable` 不填值;文件变更标"已过期" |
| 数值与表格核对 | 已做、未验证 | `research_check_number`、`research_check_table` |
| 扫描版 OCR(仅 macOS) | 已做 | 系统 Vision 本机识别;只在系统生成的单页英文样本上跑通,中文/多栏/表格/倾斜未验证 |
| 聊天里的引用点开阅读器 | 已做、**未验证**(3.0.0.137) | `[anchor N]`/`(anchor N)` 渲染成可点的标记,Academic 项目标签页里点开阅读器并定位;提示词要求模型把锚点留在论断旁 |

### CLI(omp TUI)

阶段 1、2、3 已做(用户消息左竖线、rail 输入框、选项卡对齐、连续探索折叠、底部信息逐级降级、右侧栏,设置 `tui.sidePanel`);会话标签条已做(3.0.0.137,内核未重编):输入框上方列出本窗口打开过的会话(两个起显示,最多 8 个),`alt+s`/`alt+shift+s` 原地切换,运行中不切,`tui.sessionTabs` 可关;阶段 4 可选。右侧栏只画在屏幕的可变区域,不写进终端回滚区。macOS Option 键映射已修。精简提示词与 lite 提示词配置(Gemini Flash/Lite)已做。均未在真实终端逐项确认。

### 已清理 / 已决定

- 2026-11-05 已清理构建缓存:删了 `target/debug`、残留 `rw.*.dmg`、已合并 agent worktree 与旧临时目录,磁盘可用 20G→59G。保留 release 增量缓存、`omp/target`、cargo registry、node_modules。
- 不换 Chromium/CEF:已评估并决定不做(见下文 2026-11-05 之后)。
- 只做 macOS。例外(2026-11-05 决定):CLI 可以做 Windows amd64 适配,改动放 `omp`;Desktop 和其他部分不做。

### 2026-11-05 之后(未提交)

- 快速询问改成悬浮面板(Option+Space):不激活 App(NSPanel)、独立持有一个普通会话、消息与工具卡片复用主窗口组件、底部胶囊切模式/模型/思考、"在窗口中继续"。页面用假后端验证,**原生部分只编译过**。详见 `desktop/docs/architecture/quick-ask.md`。
- Code 对标 Codex/Claude Code 第一批:每个任务一个 worktree(`worktree.rs`,放在 `~/.local/share/npi/worktrees/`,侧栏分支按钮新建,标签归在原项目下,Git 浏览器里合并、开 PR、删除);diff 行评论一次发给 Agent、单处改动撤销(`git_browser/hunk.rs`);Rewind 可同时还原文件(在内核 `session/file-checkpoints.ts`,CLI 和 Desktop 共用,存 `<agent 目录>/checkpoints/`,发提示前快照,最多等 2 秒,保留 200 个,上限 2 GiB;Desktop 用 RPC `plan_file_restore`、`restore_files`,CLI 用 Esc Esc)。Rust 用真实 git 测试,界面用假后端在浏览器验证。**未在真实 App 里验证**:新建 worktree 标签、Rewind 还原、评论发送。详见 `desktop/docs/architecture/worktree-tasks.md`。
- **用户已实测通过(3.0.0.124):** Reader 看 PDF 页、右侧文献栏、macOS 通知授权。其余项(快速询问、Cmd+W、独立窗口、Chat 转交、左图右文、图片 429、OCR、摘要与对照表、MCP)仍在测试中。
- 2026-11-06 接手 `archive/HANDOFF_3.0.0.136.md`：两个失败测试及交接涉及的新增 lint 已修；定向 Rust 42 项、内核排班相关 227 项、前端 6 项检查通过。浏览器实际组件验证评审接受/拒绝与技能 Undo 回调；重建内核 CLI 配置读写和 RPC `get_state` 通过，随包 App 的真实 MCP 进程列出两个工具并在未确认时拒绝评审。完整 dmg 已重建、稳定签名、镜像与挂载后签名校验通过；之后用户安装完整包完成真机验收,已提交(未推送)。nightly 缺 clippy；现有工具链严格全仓 lint 仍有范围外诊断，不能声称全仓 lint 通过。

- 阅读器改为看 PDF 原页(PDF.js 4.10.38,Apache-2.0,已入库 `src/vendor/pdfjs/`):文字层可选、缩放、页码框;搜索命中与锚点跳转到页并标出引文;"文字"标签保留。
- OCR 工具:`research_ocr`(Copilot)、`academic_ocr`(Academic 主标签),每次最多 6 页;宿主工具调用改在独立线程执行。
- Academic 顶栏默认显示"文献库""阅读器"和"文献栏"开关三个按钮;最右侧新增文献栏(搜索、分组、在提示词里引用、在文献库中打开),默认显示。
- Librarian 完善批:切换/关闭/附件/建议都先保存表单不丢改动;多选与批量(分组、标签、对齐、回收、两条合并);内置筛选(缺元数据、无 PDF、可能重复、待处理建议);可改 citekey 与复制 `[@key]`;元数据对齐面板;导入面板支持粘贴 DOI/arXiv 与 PubMed、Web of Science、EndNote XML(按内容识别);OpenAlex 兜底 Crossref;多选/拖入 PDF 排队;找回缺失附件;PDF 入库后自动索引。真实应用内未验证:原生拖放、文件对话框、带未保存内容关窗;OpenAlex 与新格式未用真实服务/真实导出验证。
- 提示词与工具一致性:提示词只写当前会话真有的东西(不存在的摘要、未建的 write/、没有的记忆工具都不再提),测试核对所有提示词、工具描述、MCP 说明和项目模板里提到的工具;移动过的附件标记缺失;App 搬家后自动更新 `.mcp.json`。
- 设置新增 Usage 页:订阅额度横向统计条(`usage_limits`,数据来自 `omp usage --json`)加原用量统计;统计面板拆成 `UsageStatsBody` 共用。额度条用假数据看过,统计嵌入设置页未在真实应用看过。
- Memory:Chat、Code、Academic 通用的记忆(`~/.local/share/npi/memory.db`),模型在对话中自动记一句,会话开始时读取;设置里默认开启,可编辑、删除。已用真实模型验证记录与使用;标签页实际启动未验证。
- Academic 提示词整理:Academic 会话原来只有 Code 的提示词,现在加了专属提示词(研究工作与证据规则)和统一的工具指南 `LIBRARY_GUIDE`;Copilot 提示词重写,测试保证指南与工具表一致;工具描述精简。
- Agent 绑定 PDF:`library_attach`(MCP 与内建工具都有),`library_add` 入库时自动绑定(可给 `pdf`,否则按 DOI/标题在 `papers/` 里匹配,多个候选时不绑)。
- 论文写作流水线:`write/` 独立 Git 仓库,Markdown 初稿 → Compile(Agent 逐文件转 LaTeX,`references.bib` 由文献库生成)→ Link(填模板输出 `output/main.tex`);Write 管理窗口带流水线进度条,可一键打开 VS Code/Typora。已用真实模型跑通 Compile 和 Link;窗口只在假后端下看过。文献栏加了 Open in Reader。详见 `desktop/docs/architecture/writing.md`。
- Copilot 能力(提前自 3.1):Copilot 可整理文献(`library_overview/group/tag/merge` 加查询、对齐、添加、编辑);`copilot_ask` 让 Academic 会话把长任务交给 Copilot,同时有 MCP 服务 `npi-copilot`(`--npi-mcp-copilot`,"连接 Agent"一并登记)。修复:omp 会对宿主工具弹审批,现由 reader 自动应答。已用真实模型跑通 `copilot_ask`(含 `set_host_tools` 握手);应用内调用和 omp 经 `.mcp.json` 连接未验证。
- Librarian MCP 服务(`--npi-mcp-librarian`,"连接 Agent"按钮登记到项目 `.mcp.json`):含联网查 Crossref 的 `library_lookup` 与对齐元数据的 `library_align`(默认只预览)。已用手写 MCP 客户端对真实 Crossref 验证;omp 实际连接未验证。
- Academic 会话内建文献库工具(`copilot::AcademicHost`:宿主工具,无需连接 MCP、无审批、不用传 project):Copilot 全套 + `library_lookup/align/add/edit`;提示词补充调用顺序与示例,错误信息写成能指导下一步;不提供删除,每次改动先备份到 `.npi-academic/agent-edits.jsonl`。MCP 服务提供同样四个工具(终端 `npi` 要免审批需配 `tools.approval`,见 `librarian.md`)。
- **不换 Chromium 底座(2026-11-05 决定)**:调研过 `tauri-runtime-cef`(官方 3.0.0-alpha.6 要升 Tauri 3 alpha;第三方 SableClient 版兼容 Tauri 2.12 但只在 Linux 验证过,macOS 未测、打包要自己写),代价大于收益,放弃。Reader 看 PDF 已由 PDF.js 解决;Browser 面板仍用随包的 `chrome-headless-shell`。
- **Windows 适配交给别人**(2026-11-05 决定):本项目只做并维护 macOS;`PLAN.md` 里与 Windows 相关的内容保留作交接参考,不再由这里推进。

## 3. 总进度

按 `PLAN.md` 的里程碑(1.1.0)看:

| 里程碑 | 内容 | 状态 |
|---|---|---|
| M0 地基 | Academic 模式、工作区、技能模板、起始页 | 已做 |
| M0.5 MCP 与技能底座 | 技能:有 4 个;Copilot 走 Host Tool RPC(`copilot.rs`);**Librarian 已有 MCP 服务**(`librarian/mcp.rs`),Research Core 的 MCP 暴露未做 | 部分 |
| M1 数学显示 | KaTeX 已打包 | 已做 |
| M2 稿件中间表示与三种格式 | `.tex`/`.md`/`.docx` 读章节、定位、写回修订 | **未做** |
| M3 LaTeX 与预览 | Tectonic、日志解析、PDF 预览 | **未做** |
| M4 中英对照翻译(旗舰) | 分段、占位符、术语表、对齐、同步滚动、导出 | **未做** |
| M5 模拟审稿与 rebuttal | | **未做** |
| M6 论文作图 | | **未做** |
| M7 本地引用管理 | Librarian + Research Core,**超出原计划** | 已做(未验证) |
| M8 打磨与发布 | | **未做** |

粗算:Research MVP(`PLAN_3.0`)代码约 90%,剩下是真机验证;`PLAN.md` 的 1.1.0 全量约三成。

## 4. 下一步(按顺序)

### 当前下一步（2026-11-14 用户指定，优先于下面各步，按顺序）

0. ~~整理汇总根目录 `docs/`~~（2026-11-14 已做，见 `docs/README.md`、`docs/RELEASES.md`）：合并重复、已完成的归入 `archive/`，保证本文仍是唯一进度入口；不动 `omp/` 与 `desktop/docs/` 的文档。
1. ~~部分 session 打开失败的修复~~（2026-11-14 已撤销）：原因是用户当时没选好权限，改正后会话正常打开，不是代码问题，无需修复。
2. **性能优化**（单列，排在修复之后）：
   - 先量后改：对长会话加载、切换标签、流式渲染、启动耗时取基线数字，再选瓶颈下手。
   - 不做没有数据支撑的优化；每项记录前后对比。

### 第一步:收口 —— 验证、修问题、提交(约 1~2 个工作段)

1. 你装 `3.0.0.121`,按下面的清单点一遍,把现象或截图发我,我修。
2. ~~修订 `PLAN.md`~~(2026-11-06 已做,见 §6)。

实测清单:

1. 通知:装新包第一次启动有没有弹允许提示;没有就看"系统设置 → 通知"里有没有 NPi Desktop。
2. Cmd+W:能否收到菜单栏图标,点图标能否叫回。Option+Space:窗口有内容、下拉菜单完整、模糊与流光效果。
3. Chat:提明显要写代码的需求,看转交卡片与新标签页的提示词;说"记住我喜欢……"看 `remember`。
4. Chat 介绍景点并配图:看模型是否改用左图右文卡片;多找几张图看 429 是否减少。
5. Academic:工具栏菜单"阅读器"→ 建索引 → 翻页/搜索/章节;让 Copilot 总结一篇、做一张对照表;扫描版 PDF 看 OCR 标记。
6. Librarian 窗口:搜索中文短词。独立窗口能缩放、Cmd+W 能关。

### 第二步:清掉遗留(约 2 个工作段)

**已清完(3.0.0.137,未验证)**:其余二级页面改独立窗口(设置除外)、聊天里的引用点开阅读器、CLI 会话标签条。Chromium 已决定不换;首字耗时埋点决定不做。

### 第三步:较小缺口(约 2 个工作段,可穿插)

**已清完**:合并重复文献、OpenAlex、EndNote XML / `.nbib` / Web of Science 导入、拖 PDF 到窗口、附件按哈希找回(3.0.0.126);无 DOI 的 PDF 让模型读首页(须用户确认,3.0.0.137,未验证;只对已入库且绑了 PDF 的条目,新导入的草稿不提供)。

### 第四步:1.1.0 主体(约 10~14 个工作段)

| 顺序 | 里程碑 | 估计 | 备注 |
|---|---|---|---|
| 1 | M2 稿件中间表示 | 2~3 段 | Word 保留样式写回最难 |
| 2 | M3 LaTeX 与预览 | 约 2 段 | 只做 macOS,比原计划轻 |
| 3 | M4 中英对照翻译 | 3~4 段 | 旗舰;可先做 Markdown/LaTeX,Word 随 M2 |
| 4 | M5 审稿与 rebuttal | 1~2 段 | |
| 5 | M6 论文作图 | 1~2 段 | |
| 6 | M8 打磨与发布 | 1~2 段 | 干净安装演练(macOS) |

每个里程碑结束:更新本文、`CHANGELOG`、发一版。

### 推迟到 3.1

`research_propose_ops`(Copilot 提结构修改,授权绑定 `planId + baseRevision`)、按论文逐篇授权、Docling PoC。

## 5. 已知风险

1. **真实模型与真实数据**:所有 Copilot 工具(含新增 5 个)、Crossref、PDF 首页规则只有单元测试;`set_host_tools` 握手和模型的实际用法是最大未知。
2. **OCR 只验证了最简单的样本**。
3. **Chat 首字速度的上限在模型通道**(`google-antigravity` 一次性返回),界面侧能做的有限。
4. 内核二进制来自 omp 提交 `f2e5ec9272`(分支 `kernel.2.0.10-universal`,未推送)。
5. **自签名证书只在本机有效**:不能分发给别人安装。

## 6. 对 `PLAN.md` 的修订(2026-11-06 已做,`PLAN.md` 升为草案 v0.3)

1. 决定表"TeX 引擎:必须兼容 Windows"改为"只做 macOS"(2026-11-04);§7 的跨平台抽象、M3 的 Windows runner 用例、M8 的 Windows 演练删去。
2. 状态栏改为"以 `docs/PROGRESS.md` 为准"。
3. M0.5 的 MCP 服务一项改写为"已用 Host Tool RPC 代替,是否仍做 MCP 另议"。
4. M7 标注"已由 Librarian + Research Core 实现"。
