# 总进度与下一步

整理于 2026-11-06(分支 `3.0.0.137`)。这是**唯一的进度入口**:其他计划文档(`PLAN.md`、`PLAN_3.0.md`、`PLAN_LIBRARIAN.md`、`PLAN_CLI.md`、`PLAN_ACADEMIC_NEXT.md`)写"打算做什么",本文写"现在到了哪里、下一步做什么"。每做完一批就回来改这里。

验证记录:用户装 `3.0.0.132` 后在真实 app 里点过上面列的功能(含 3.0.0.122–132 新增部分),反馈没有问题;2026-11-06 用户装 `3.0.0.136` 完整包完成验收(会话搜索、画像、技能撤销、模型排班、多专家评审)。逐行的"未验证"标记保留原样,没有逐条改,以后遇到具体问题再回写。

状态词:**已做**=代码和单元测试完成;**已验证**=在真实 app / 真实数据里确认过;**未验证**=只有编译与单元测试。

## 0. 先看这三件事

1. **已提交并推送(2026-11-05)。** Desktop 分支 `3.0.0.121` 已提交(`bb1d949` 功能、`5f36015` 文档、`36053d3` 版本号)并推送到 `origin/3.0.0.121`;`3.0.0.101`(`bb79b6e`)同时推送。`3.0.0.102`、`3.0.0.103` 与 101 指向同一个提交,没有推送(没有独立内容)。omp 仓库 `kernel.2.0.10-universal` 提交 `52a22ce3e0` 并推送到 `NPi-Kernel`。
2. **两个仓库都已提交并推送**:Desktop 分支 `3.0.0.132`(含 3.0.0.122–132 的全部提交),`omp` 的 `kernel.2.0.10-universal`。 之后的改动从 `3.0.0.121` 继续,版本号要升时新建分支。
3. **平台范围:只做 macOS**(2026-11-04 决定);**Windows 适配交给别人**(2026-11-05)。`PLAN.md` 里"必须兼容 Windows"的要求在本仓库作废(见 §6),相关内容留作交接参考。

## 1. 版本与产物

| 版本 | 状态 | dmg(`dist/<版本>/`) | 签名 | 说明 |
|---|---|---|---|---|
| 3.0.0.52 | 已提交、dmg 已有 | `3.0.0.52/` | 临时 | CLI 改版与工具输出预算 |
| 3.0.0.72 | 已提交并已推送 | — | — | Librarian L2/L3、图片删除修复 |
| 3.0.0.101 | 已提交并推送(`bb79b6e`) | `3.0.0.101/` | 临时 | Research Core、快速询问、托盘 |
| 3.0.0.102 | 与 101 同一提交(内容在 121 里) | `3.0.0.102/` | 临时 | Cmd+W、独立窗口、Chat 裁剪与转交 |
| 3.0.0.103 | 与 101 同一提交(内容在 121 里) | `3.0.0.103/` | **稳定证书** | 加左图右文、图片 429、快速询问玻璃、新内核 |
| 3.0.0.121 | 已提交并推送(`bb1d949`) | `3.0.0.121/` | **稳定证书** | 加 Academic 阅读器、摘要与对照表、数值核对、OCR |
| 3.0.0.122–125 | 内容并入 126(中间版本,dmg 已有) | `3.0.0.12x/` | **稳定证书** | PDF 阅读器、OCR 工具、内建文献库工具、Copy as…、Copy 修复 |
| 3.0.0.126 | 已提交(`4ca309d`,未推送) | `3.0.0.126/` | **稳定证书** | Librarian 完善:多选与批量、合并、对齐面板、导入面板与新格式、OpenAlex、拖拽入组、找回附件;SHA-256 `74780a8a…8cf2`(未在真机实测) |
| 3.0.0.127 | 内容并入 130 | `3.0.0.127/` | **稳定证书** | Copilot 整理文献、`copilot_ask` 与 `npi-copilot` MCP、宿主工具审批自动应答 |
| 3.0.0.128 | 内容并入 130 | `3.0.0.128/` | **稳定证书** | Write 流水线、文献栏 Open in Reader |
| 3.0.0.129 | 内容并入 130 | `3.0.0.129/` | **稳定证书** | Agent 绑定 PDF、Academic 提示词与工具整理、Librarian UI 修复 |
| 3.0.0.130 | 已提交(`aa7753b`) | `3.0.0.130/` | **稳定证书** | Memory、设置里的 Usage 页;SHA-256 `80660874…8713`(未在真机实测) |
| 3.0.0.131 | 内容并入 132 | `3.0.0.131/` | **稳定证书** | 设置左侧菜单加入 Memory 与 Usage(原先写了配置但没进菜单) |
| 3.0.0.132 | 已提交并推送 | `3.0.0.132/` | **稳定证书** | 内核 `read-path.md` 修复、提示词/工具/MCP 与实际一致、附件缺失标记;SHA-256 `651cc8bd…8509`(未在真机实测) |
| 3.0.0.133 | 内容并入 135 | `3.0.0.133/` | **稳定证书** | worktree 任务、diff 行评论与单处撤销、Rewind 还原文件(内核)、快速询问悬浮面板 |
| 3.0.0.134 | 内容并入 135 | `3.0.0.134/` | **稳定证书** | 内核含 CLI 界面改版与阿里四个入口的模型信息、思考等级;`alibaba` 显示为国际站;SHA-256 `caa0fed5…863d`(未在真机实测) |
| 3.0.0.135 | 已提交并推送(分支 `3.0.0.135`) | `3.0.0.135/` | **稳定证书** | 内核含 CLI 右侧栏额度、设置页二级菜单、面板式弹窗;Desktop 前端相对 134 未改;SHA-256 `dae2b473…a1f5`(未在真机实测) |
| 3.0.0.136 | 内容并入下一行完整包 | — | **稳定证书** | 目录整理:配置跟随 CLI 到 `~/.local/share/npi/desktop`,缓存统一到 `~/.cache/npi`(含内核缓存与 Rewind 快照),worktree 到 `~/Documents/NPi/Code/.worktrees`、`.agent-worktrees`;界面设置写一份到 `ui-state.json`;首次启动自动迁移;旧 dmg 已被完整包替换 |
| 3.0.0.136 完整包 | 已提交并推送(`f78c829`)、**已验证**(用户真机验收,2026-11-06) | `3.0.0.136/NPi Desktop_3.0.0.136_aarch64.dmg` | **稳定证书** | 包含统一记忆、会话搜索、分模式画像、技能撤销、模型排班表、多专家模拟评审；278876587 字节；SHA-256 `9f6d6b92d4be551de6dc9e4f138c1b496a206e721dccf99aac6ca1b26854d809`；已替换仅含目录整理的旧 dmg |
| 3.0.0.137 | 已提交并推送(`b046186`,分支 `3.0.0.137`) | `3.0.0.137/` | **稳定证书** | 清遗留:聊天里 `[anchor N]` 点开阅读器;Librarian"让模型读第 1 页";历史会话、审批规则、Plan、Git 浏览器改独立窗口(`app/panel-link.js`);CLI 会话标签条(内核 `bcbabfad9f`);`PLAN.md` 修订;278924606 字节;SHA-256 `84cf9c8075b63e14393d86cd2677caa4798bcb0901e833c7040a86a5c15fdec9`(未在真机实测) |
| 3.0.0.138 | 已提交(分支 `3.0.0.138`)、**已验证**(用户真机测试,2026-11-07) | `3.0.0.138/NPi Desktop_3.0.0.138_aarch64.dmg` | **稳定证书** | Option+Space 弹窗改用 macOS 原生 Liquid Glass(`objc2-app-kit` 的 `NSGlassEffectView` + `NSGlassEffectContainerView`,卡片与模式胶囊各一块,macOS 26 前退回 `NSVisualEffectView`);页面不再自绘玻璃;去掉输入框蓝色焦点环;内核未变;277359635 字节;SHA-256 `df095d3eaa1a4bf5ecff4a4bb116c68a1279d0b85cbd3806c9f924de71cb1bea` |

打包流程(每次都这样,别改顺序):

1. `node scripts/build-frontend.mjs`
2. `devstore run rust/current -- /usr/bin/env PATH="<nvm node>:$PATH" node node_modules/.bin/tauri build --config src-tauri/tauri.dist.conf.json --bundles app`(不要带 `tauri.localsign.conf.json`:`devstore run` 改了 `HOME`,看不到登录钥匙串,会报 "The specified item could not be found in the keychain")
3. 在 devstore 之外:`codesign --force --deep --options runtime --sign "NPi Desktop Local" "<.app>"`
4. `bundle_dmg.sh --skip-jenkins --volname "NPi Desktop" --app-drop-link 480 170 <输出> <含 .app 的目录>`(Tauri 自带的 dmg 步骤会被 Finder 拒绝)
5. `hdiutil verify`、挂载后 `codesign --verify --deep --strict`、`shasum -a 256`
6. 把 dmg 放进 `dist/<版本>/`,校验通过后删除 `target/release/bundle/` 下的源 dmg、`.app` 和 `rw.*.dmg`,只保留 `dist/` 里的那一份(增量编译缓存不动;`bundle/dmg/bundle_dmg.sh` 与 `icon.icns` 保留,只打 app 时 Tauri 不会重新生成它们,第 4 步要用)

内核:`omp/packages/coding-agent` 里 `bun run build`(bun 在 `.tools/node_modules/bun/bin`),产物 `dist/npi` 复制到 `desktop/src-tauri/binaries/npi-aarch64-apple-darwin`。最近一次编译于 2026-11-06 14:42(omp `bcbabfad9f`)。

第 2 步的 `devstore run` 要保留环境注入的 PATH:`devstore run rust/current -- sh -c 'PATH="<nvm node>:$PATH" exec node node_modules/.bin/tauri build …'`。DevStore 若因卷意外断开留下过期租约(`mount` 报 `no lease on the image`,`unmount` 也失败),把映像目录里的 `lease.json` 改名后再挂载(2026-11-06 遇到过,是 DevStore 的 bug)。

稳定签名证书 `NPi Desktop Local`(自签名,有效期到 2036 年,在登录钥匙串)用于保住通知授权;换电脑、重置钥匙串或重装系统后要重跑 `scripts/macos-local-signing.sh`。建议把它连私钥导出成 `.p12` 备份。

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
- 2026-11-06 接手 `HANDOFF_3.0.0.136.md`：两个失败测试及交接涉及的新增 lint 已修；定向 Rust 42 项、内核排班相关 227 项、前端 6 项检查通过。浏览器实际组件验证评审接受/拒绝与技能 Undo 回调；重建内核 CLI 配置读写和 RPC `get_state` 通过，随包 App 的真实 MCP 进程列出两个工具并在未确认时拒绝评审。完整 dmg 已重建、稳定签名、镜像与挂载后签名校验通过；之后用户安装完整包完成真机验收,已提交(未推送)。nightly 缺 clippy；现有工具链严格全仓 lint 仍有范围外诊断，不能声称全仓 lint 通过。

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
4. 内核二进制来自 omp 提交 `b2b044ce14`(分支 `kernel.2.0.10-universal`,已推送到自己的 `origin`)。
5. **自签名证书只在本机有效**:不能分发给别人安装。

## 6. 对 `PLAN.md` 的修订(2026-11-06 已做,`PLAN.md` 升为草案 v0.3)

1. 决定表"TeX 引擎:必须兼容 Windows"改为"只做 macOS"(2026-11-04);§7 的跨平台抽象、M3 的 Windows runner 用例、M8 的 Windows 演练删去。
2. 状态栏改为"以 `docs/PROGRESS.md` 为准"。
3. M0.5 的 MCP 服务一项改写为"已用 Host Tool RPC 代替,是否仍做 MCP 另议"。
4. M7 标注"已由 Librarian + Research Core 实现"。
