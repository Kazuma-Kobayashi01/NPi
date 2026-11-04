# 总进度与下一步

整理于 2026-11-05(分支 `3.0.0.121`)。这是**唯一的进度入口**:其他计划文档(`PLAN.md`、`PLAN_3.0.md`、`PLAN_LIBRARIAN.md`、`PLAN_CLI.md`、`PLAN_ACADEMIC_NEXT.md`)写"打算做什么",本文写"现在到了哪里、下一步做什么"。每做完一批就回来改这里。

状态词:**已做**=代码和单元测试完成;**已验证**=在真实 app / 真实数据里确认过;**未验证**=只有编译与单元测试。

## 0. 先看这三件事

1. **已提交并推送(2026-11-05)。** Desktop 分支 `3.0.0.121` 已提交(`bb1d949` 功能、`5f36015` 文档、`36053d3` 版本号)并推送到 `origin/3.0.0.121`;`3.0.0.101`(`bb79b6e`)同时推送。`3.0.0.102`、`3.0.0.103` 与 101 指向同一个提交,没有推送(没有独立内容)。omp 仓库 `kernel.2.0.10-universal` 提交 `52a22ce3e0` 并推送到 `NPi-Kernel`。
2. **`omp` 仓库干净;Desktop 的 3.0.0.122–126 改动已在分支 `3.0.0.126` 提交,未推送。** 之后的改动从 `3.0.0.121` 继续,版本号要升时新建分支。
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
| 3.0.0.126 | 已提交(未推送,当前) | `3.0.0.126/` | **稳定证书** | Librarian 完善:多选与批量、合并、对齐面板、导入面板与新格式、OpenAlex、拖拽入组、找回附件;SHA-256 `74780a8a…8cf2`(未在真机实测) |

打包流程(每次都这样,别改顺序):

1. `node scripts/build-frontend.mjs`
2. `devstore run rust/current -- /usr/bin/env PATH="<nvm node>:$PATH" node node_modules/.bin/tauri build --config src-tauri/tauri.dist.conf.json --bundles app`(不要带 `tauri.localsign.conf.json`:`devstore run` 改了 `HOME`,看不到登录钥匙串,会报 "The specified item could not be found in the keychain")
3. 在 devstore 之外:`codesign --force --deep --options runtime --sign "NPi Desktop Local" "<.app>"`
4. `bundle_dmg.sh --skip-jenkins --volname "NPi Desktop" --app-drop-link 480 170 <输出> <含 .app 的目录>`(Tauri 自带的 dmg 步骤会被 Finder 拒绝)
5. `hdiutil verify`、挂载后 `codesign --verify --deep --strict`、`shasum -a 256`

内核:`omp/packages/coding-agent` 里 `bun run build`(bun 在 `.tools/node_modules/bun/bin`),产物 `dist/npi` 复制到 `desktop/src-tauri/binaries/npi-aarch64-apple-darwin`。最近一次编译于 2026-11-04 10:47,包含图片 `description` 字段。

稳定签名证书 `NPi Desktop Local`(自签名,有效期到 2036 年,在登录钥匙串)用于保住通知授权;换电脑、重置钥匙串或重装系统后要重跑 `scripts/macos-local-signing.sh`。建议把它连私钥导出成 `.p12` 备份。

## 2. 已做的工作(按领域)

### Desktop 基础与体验

| 项 | 状态 | 说明 |
|---|---|---|
| 快速询问(Option+Space) | 已做、**未验证** | 根因是导航防护拦了它的页面,已放行;自绘玻璃 + 流光 + 系统模糊底层;下拉菜单在条下方,窗口随之增高 |
| Cmd+W 收到菜单栏图标 | 已做、**未验证** | 根因是快捷键表把 Cmd+W 绑成了"关闭标签页";现在 Cmd+W 走 `hide_main_window`,关闭标签页只剩 Ctrl+W |
| 菜单栏图标与 Dock 切换 | 已做、**未验证** | 用项目 logo;窗口隐藏时 Dock 图标消失 |
| 二级页面独立窗口 | **部分** | 已做:Librarian、阅读器、用量统计(`panel_window.rs`,页面入口 `?panel=`,`live.js` 在面板里不启动)。**未做**:设置、Git 浏览器、Plan、审批规则、历史会话(依赖主窗口的实时状态,需要先做跨窗口通信) |
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
| 首字耗时埋点 | **未做** | 可用 `~/.omp/stats.db` 的 `ttft` 对比 |

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
| 聊天里的引用点开阅读器 | **未做** | 能力(`anchor` 参数)有了,聊天消息里没入口 |

### CLI(omp TUI)

阶段 1、2 已做、阶段 3 部分已做(用户消息左竖线、状态行上下文用量、rail 输入框、选项卡对齐);**未做**:右侧 Context 栏、会话标签条;阶段 4 可选。macOS Option 键映射已修。精简提示词与 lite 提示词配置(Gemini Flash/Lite)已做。均未在真实终端逐项确认。

### 已清理 / 已决定

- 2026-11-05 已清理构建缓存:删了 `target/debug`、残留 `rw.*.dmg`、已合并 agent worktree 与旧临时目录,磁盘可用 20G→59G。保留 release 增量缓存、`omp/target`、cargo registry、node_modules。
- 不换 Chromium/CEF:已评估并决定不做(见下文 2026-11-05 之后)。
- 只做 macOS。

### 2026-11-05 之后(未提交)

- **用户已实测通过(3.0.0.124):** Reader 看 PDF 页、右侧文献栏、macOS 通知授权。其余项(快速询问、Cmd+W、独立窗口、Chat 转交、左图右文、图片 429、OCR、摘要与对照表、MCP)仍在测试中。

- 阅读器改为看 PDF 原页(PDF.js 4.10.38,Apache-2.0,已入库 `src/vendor/pdfjs/`):文字层可选、缩放、页码框;搜索命中与锚点跳转到页并标出引文;"文字"标签保留。
- OCR 工具:`research_ocr`(Copilot)、`academic_ocr`(Academic 主标签),每次最多 6 页;宿主工具调用改在独立线程执行。
- Academic 顶栏默认显示"文献库""阅读器"和"文献栏"开关三个按钮;最右侧新增文献栏(搜索、分组、在提示词里引用、在文献库中打开),默认显示。
- Librarian 完善批(分支 `3.0.0.125`,未提交):切换/关闭/附件/建议都先保存表单不丢改动;多选与批量(分组、标签、对齐、回收、两条合并);内置筛选(缺元数据、无 PDF、可能重复、待处理建议);可改 citekey 与复制 `[@key]`;元数据对齐面板;导入面板支持粘贴 DOI/arXiv 与 PubMed、Web of Science、EndNote XML(按内容识别);OpenAlex 兜底 Crossref;多选/拖入 PDF 排队;找回缺失附件;PDF 入库后自动索引。真实应用内未验证:原生拖放、文件对话框、带未保存内容关窗;OpenAlex 与新格式未用真实服务/真实导出验证。
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
2. 修订 `PLAN.md`(见 §6)。(提交与推送已完成。)

实测清单:

1. 通知:装新包第一次启动有没有弹允许提示;没有就看"系统设置 → 通知"里有没有 NPi Desktop。
2. Cmd+W:能否收到菜单栏图标,点图标能否叫回。Option+Space:窗口有内容、下拉菜单完整、模糊与流光效果。
3. Chat:提明显要写代码的需求,看转交卡片与新标签页的提示词;说"记住我喜欢……"看 `remember`。
4. Chat 介绍景点并配图:看模型是否改用左图右文卡片;多找几张图看 429 是否减少。
5. Academic:工具栏菜单"阅读器"→ 建索引 → 翻页/搜索/章节;让 Copilot 总结一篇、做一张对照表;扫描版 PDF 看 OCR 标记。
6. Librarian 窗口:搜索中文短词。独立窗口能缩放、Cmd+W 能关。

### 第二步:清掉遗留(约 2 个工作段)

1. 其余二级页面改独立窗口(先设计跨窗口通信:事件 + 命令)。
2. ~~评估改用 Chromium~~(已决定不做,见上)。
3. 聊天里的引用点开阅读器(`OMP_OPEN_PANEL("reader", { anchor })`)。
4. 首字耗时埋点(可选)。

### 第三步:较小缺口(约 2 个工作段,可穿插)

合并重复文献、OpenAlex、EndNote XML / `.nbib` / Web of Science 导入、拖 PDF 到窗口、附件按哈希找回、无 DOI 的 PDF 让模型读首页(须用户确认)。

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

`research_propose_ops`(Copilot 提结构修改,授权绑定 `planId + baseRevision`)、按论文逐篇授权、从主 Chat 调用 Copilot、Docling PoC。

## 5. 已知风险

1. **真实模型与真实数据**:所有 Copilot 工具(含新增 5 个)、Crossref、PDF 首页规则只有单元测试;`set_host_tools` 握手和模型的实际用法是最大未知。
2. **OCR 只验证了最简单的样本**。
3. **Chat 首字速度的上限在模型通道**(`google-antigravity` 一次性返回),界面侧能做的有限。
4. 内核二进制是从 omp 提交 `52a22ce3e0` 的工作树编译的(之后 omp 没再改)。
5. **自签名证书只在本机有效**:不能分发给别人安装。

## 6. 对 `PLAN.md` 需要的修订(待做)

1. 决定表"TeX 引擎:必须兼容 Windows"改为"只做 macOS"(2026-11-04);§7 的跨平台抽象、M3 的 Windows runner 用例、M8 的 Windows 演练删去。
2. 状态栏改为"以 `docs/PROGRESS.md` 为准"。
3. M0.5 的 MCP 服务一项改写为"已用 Host Tool RPC 代替,是否仍做 MCP 另议"。
4. M7 标注"已由 Librarian + Research Core 实现"。
