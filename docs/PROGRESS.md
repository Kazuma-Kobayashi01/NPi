# 总进度与下一步

整理于 2026-11-06(分支 `3.0.0.137`)。这是**唯一的进度入口**:`PLAN.md`(1.1.0 规划,唯一仍在推进的计划)写"打算做什么";已完成的计划与交接记录在 [`archive/`](archive/README.md),本文写"现在到了哪里、下一步做什么"。每做完一批就回来改这里。

验证记录:用户装 `3.0.0.132` 后在真实 app 里点过上面列的功能(含 3.0.0.122–132 新增部分),反馈没有问题;2026-11-06 用户装 `3.0.0.136` 完整包完成验收(会话搜索、画像、技能撤销、模型排班、多专家评审)。逐行的"未验证"标记保留原样,没有逐条改,以后遇到具体问题再回写。

状态词:**已做**=代码和单元测试完成;**已验证**=在真实 app / 真实数据里确认过;**未验证**=只有编译与单元测试。

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
| 3.0.0.138 CLI loading | 已构建，内核已提交推送(`09930e2f0e`)、未安装(2026-11-07) | `3.0.0.138/NPi Desktop_3.0.0.138_cli-loading_aarch64.dmg` | **稳定证书** | 重编译内核 `npi/2.0.10`，CLI 等待行随机选择 25 组颜文字与英文短语，流光跟随主题 accent 高光；277357521 字节；SHA-256 `f39c0bfaec099ecbdefb7ba3bb238c00beeafc62129d7401dae090244dc9acea`；镜像校验、挂载后深度签名校验及包内内核隔离 `get_state` 启动冒烟通过，无模型请求；保留原版 138 dmg，未替换正在使用的应用 |
| 3.0.0.138 读图与登录页 | 已构建、已提交推送(内核 `31660c16f6`)，待用户真机试用(2026-11-07) | `3.0.0.138/NPi Desktop_3.0.0.138_vision-login_aarch64.dmg` | **稳定证书** | 重编译内核(含 CLI loading 改动)：OAuth 回调页改为 NPi 标识与中英文文案；`images.blockImages` 移到"Model → Vision"并改名 "Block Image Input"。读图不出图的根因是用户配置 `images.blockImages: true`，已改为 `false`；277358286 字节；SHA-256 `bec8e7018e1f4399db2af0658dc4fd43234a223be4308988c7061df11d44ee13`；镜像校验与挂载后深度签名校验通过 |
| 3.0.0.138 生图卡片与 Chat 接入 | 已构建，待用户真机试用(2026-11-08) | `3.0.0.138/NPi Desktop_3.0.0.138_aarch64.dmg` | **稳定证书** | 接入 `generate_image`（含 `write xd://generate_image`）结果卡片缩略图、灯箱、路径、模型/尺寸/质量与失败态；Chat 模式在会话配置 `generate_image.enabled: true` 时放行 `generate_image`；全局配置已开启并通过 Codex Plus 实测生成 WebP；277363194 字节；SHA-256 `f90258c7c7fb0d895deea6340611bbb558af4bedaa117c09521c37ea4c7dc2ba`；镜像校验、挂载后深度签名校验及包内内核隔离 `get_state` 启动冒烟通过，保留 `vision-login` dmg，未替换正在使用的应用 |
| 3.0.0.138 CLI themes | 已构建、未安装(2026-11-08) | `3.0.0.138/NPi Desktop_3.0.0.138_cli-themes_aarch64.dmg` | **稳定证书** | 重编译内核 `npi/2.0.10`，启动 Logo／字标／流光跟随主题，新增三款 `dark-tokyo-{neon,rain,chrome}`；277372924 字节；SHA-256 `df33e96c41b0abc6885c2c26347ced2c9d7158c0fa174d296d60490f98250ebf`；镜像校验、挂载后深度签名校验和包内内核隔离 `get_state` 通过，无模型请求；保留原有 dmg，已清理 app 中间产物，未替换正在使用的应用 |
| 3.0.0.138 CLI Wide | 已构建、未安装(2026-11-08) | `3.0.0.138/NPi Desktop_3.0.0.138_cli-wide_aarch64.dmg` | **稳定证书** | 重编译内核 `npi/2.0.10`，Content Width 命名档位 Common（100 列）／Wide（140 列），保留数字配置与默认 100 列；277373974 字节；SHA-256 `8974d327a9b7e25e2e5713727cdd018203fe461855e22b4e3962e98fabc4b9ac`；镜像校验、挂载后深度签名校验、包内内核隔离 `get_state` 通过，无模型请求；保留已有 dmg，清理 app 中间产物，未替换已安装应用 |
| 3.0.0.138 Collapse | 已构建、未安装(2026-11-08) | `3.0.0.138/NPi Desktop_3.0.0.138_collapse_aarch64.dmg` | **稳定证书** | 重编译内核 `npi/2.0.10`：Appearance → Display 新增 Collapse 四档（Off / Thinking / Thinking + Tools / All，默认 All），取代 `expandThinkingBlocks`，Ctrl+T／Ctrl+O 仅作用于当前会话；CLI 等待行全程保持颜文字动画，工具意图只显示在工具卡片；含 API Key 登录不自动打开网页、LM Studio v1 模型元数据与 Codex `gpt-6.1+` 上下文规则；277407320 字节；SHA-256 `7bca41b417037f72f918485b7ee8160bd2bf1871e0d0a483e58dfe63581dbe6b`；镜像校验、挂载后深度签名校验（`NPi Desktop Local`）、包内内核隔离 `get_state` 及 `display.collapse` 读取通过，无模型请求；已清理 app 中间产物，未替换已安装应用 |
| 3.0.0.138 Sandbox | 已构建、未安装(2026-11-08) | `3.0.0.138/NPi Desktop_3.0.0.138_sandbox_aarch64.dmg` | **稳定证书** | 进程沙盒：内核 `sandbox.mode` / `--sandbox`（off / workspace-write / read-only），macOS 下以 Seatbelt 重启内核，命令、eval、文件工具、MCP 只能写项目、npi 目录与临时目录；Desktop 设置 → Approvals → Sandbox 读写同一设置（`sandbox_config.rs`），CLI `/settings` 在 Interaction → Approvals。同包含：科研 Python 套件离线 wheelhouse（numpy/scipy/matplotlib/pandas/sympy，+76 MB）、`python` 工具降为 `xd://`、连续连接失败只重试 3 次、Flash lite 提示词批量调用；重编译内核 `npi/2.0.10`；353382211 字节；SHA-256 `efc86c34b0fa18d323fb216e4f0ee5e02555eea0c3b25836ff2873590f58a816`；镜像校验、挂载后深度签名校验、包内内核 `--sandbox workspace-write` 隔离 `get_state` 通过，无模型请求；未替换已安装应用 |
| 3.0.0.201 | 已构建、已提交(分支 `3.0.0.201`,未推送)、未安装(2026-11-09) | `3.0.0.201/NPi Desktop_3.0.0.201_aarch64.dmg` | **稳定证书** | 重编译内核 `npi/2.0.10`:CLI 输入框打字时按提示词历史前缀补全整行(当前项目优先,淡色 ghost text),行尾 → 或 Tab 接受、不发送;设置 `composer.historySuggestions`(Interaction → Input → History Suggestions,默认开);内核提交 `f1f35a53fa`(`omp` `kernel.2.0.10-universal`,未推送)。含 138 Sandbox 包的全部内容;353392036 字节;SHA-256 `8c00bbe31d89d47b5fea66ee6bdc017b29f17489192c826861d03ede8551313f`;镜像校验、挂载后深度签名校验通过,包内内核含新设置;未在真机试用,未替换已安装应用 |
| 3.0.0.201 Context | 已构建、已提交(分支 `3.0.0.201`,未推送)、未安装(2026-11-09) | `3.0.0.201/NPi Desktop_3.0.0.201_context_aarch64.dmg` | **稳定证书** | 右侧栏 Context 卡片加 Details:Context details 面板(消息构成细分、每次请求上下文柱状图与缓存命中率、工具/命令/文件统计),纯前端从现有 `get_messages` 上下文计算(`app/context-insight.js`、`design/context-insight-modal.jsx`),思路参考 dsh-context(Apache-2.0,未复用代码);同日重打:修命令解析(引号、转义空格、heredoc、`2>&1`)、消息构成按比例对齐总数(不再有约一半的"Overhead")、趋势图标注"last 60 of N"与首尾上下文量;再次重打:参考 dsh-context 改版(KPI 磁贴、构成环形图与图例条、带坐标轴/轮次分段/悬停详情的趋势图、工具与命令标签、文件读写标记),面板可拖角调整大小、默认约 1120×900 并记住尺寸(`npi.contextDetails.size`);内核未变(仍为 201 的 `npi/2.0.10`);`npm test` 全部通过;353389313 字节;SHA-256 `ee0bcc06b34d722a53345fd7baa78dd2f29bda6f01a0c4d896a283c3f8f5be1f`;镜像校验、挂载后深度签名校验通过;未在真机试用,保留 201 原 dmg,未替换已安装应用 |
| 3.0.0.201 Web search | 已构建、已提交(Desktop 分支 `3.0.0.201`、内核 `d3cb82e110`,均未推送)、未安装(2026-11-09) | `3.0.0.201/NPi Desktop_3.0.0.201_websearch_aarch64.dmg` | **稳定证书** | 含 Context 版全部内容;重编内核 `npi/2.0.10` 修复 Google 搜索:Google 结果改用 `/goto?url=` 跳转链接后解析全部丢弃,现识别并逐条跟一次跳转取真实网址(未跳转的保留 `/goto`);用户配置 `modelRoles.web: web/google,web/hosted,web/exa`(Google 优先,失败按内核默认顺序兜底);353389704 字节;SHA-256 `dd0b39a280c30ae0aa260b4cf2985481242ea0c27c5a81905205a7c283026bc8`;镜像校验、挂载后深度签名校验通过,包内内核含修复;未在真机试用,未替换已安装应用 |
| 3.0.0.201 Night city | 已构建、已提交(Desktop 分支 `3.0.0.201`、内核 `0a326b29b0`,均未推送)、未安装(2026-11-10) | `3.0.0.201/NPi Desktop_3.0.0.201_nightcity_aarch64.dmg` | **稳定证书** | 含 Web search 版全部内容;重编内核 `npi/2.0.10`:CLI 首页全屏动态背景(日本夜城视差左移:远景楼群、东京塔、霓虹招牌楼群、透视网格地面、故障扫描线,上层自动游玩的半块像素横版弹幕射击,含敌方旋转激光与我方散射激光;色调仅比背景浅一点,跟随主题),Logo 渐变缓慢流动;BGM:交互式 TUI 启动循环播放 `<agent dir>/bgm/*.mp3`(macOS `afplay`),设置 `startup.bgm` 可关,音乐不入仓库;`/rank [day|week|month|year]` 全屏 Token 排行榜(周期标签页 ←/→ 切换、↑/↓ 滚动、Enter 查看输入/输出/缓存堆叠条明细、r 刷新、Esc 关闭,单位 M Token/周期,周期从当日 0 点、周一、月初、年初起算);353647478 字节;SHA-256 `1e9936b2d41e3073a2f035b0dae0bf91530db88e6fc51e77e98375d8a025cca2`;镜像校验、挂载后深度签名校验通过,包内内核含新功能;未在真机试用桌面端,未替换已安装应用 |
| 3.0.0.201 Office | 已构建、已提交(Desktop 分支 `3.0.0.201`、内核 `b2b2cc04fd`,均未推送)、未安装(2026-11-10) | `3.0.0.201/NPi Desktop_3.0.0.201_office_aarch64.dmg` | **稳定证书** | 含 Night city 版全部内容;重编内核 `npi/2.0.10`:Virtual Office(`/office on`,主聊天为总经理,Team/Studio/Enterprise 对应 1/3/6 个 Office,每个 Office 1 名 Leader + 2–5 名 Worker,固定名 `Worker-xxxxxx`、可指定模型与能力;各成员独立分支与 worktree,只能经本地 PR 逐级审核合并,最终合并到当前分支须用户 `/office merge`);Office 按项目记忆(合并日志 + `remember` 经验,派工时带给成员);RPC `get_office_state` / `office_command` 供 GUI 接入;用户目录启动 CLI 默认进入 `~/Documents/NPi/Code`;同日清空 `/tmp` 项目的 npi 记忆;离线模拟模型跑通完整派工,未用真实模型试运行;353586198 字节;SHA-256 `5816ad3d07cfc0fb41e333b1f45dbde789e1e9097a52a24cfb57ffc65cacad56`;镜像校验、挂载后深度签名校验(`NPi Desktop Local`)通过,包内内核含新功能;桌面端尚无 Office 界面,未替换已安装应用 |
| 3.0.0.201 Office GUI | 已构建、已提交(Desktop `4ef77c1`、内核 `0b2e58fea3`,均未推送)、未安装(2026-11-10) | `3.0.0.201/NPi Desktop_3.0.0.201_officegui_aarch64.dmg` | **稳定证书** | 含 Office 版全部内容;重编内核 `npi/2.0.10`:CLI 无参数 `/office` 打开全屏面板(方向键/回车/Esc,文字版改为 `/office status`);RPC 会话默认关闭 Office、`set_office_enabled` 按会话开关,`office_state` 帧推送变化,`OfficeState` 增加各 Office 阶段、按本地日期的成员用量(含总经理)与派工记录,工位改为跨进程锁;桌面端 Virtual Office 窗口:侧边栏 Office 项与标签页右键菜单按标签页开关(随 `open-tabs.json` 恢复),Cmd+Shift+O 打开独立窗口,加油机式计量器(USD/JPY,汇率由 Rust 从 Frankfurter 每日获取并缓存)、PR 漏斗、Office 卡片,点开为每名成员一列的只读对话;合并/退回、名册、记忆均经 `office_command`;子代理订阅级别改为按窗口登记取最高;353673778 字节;SHA-256 `0f7e6ba371c552f195d3bbcc5b987b95bbc276a45bd1d17b1c1a5067fa4a7e79`;镜像校验、挂载后深度签名校验(`NPi Desktop Local`)通过,包内内核含新命令;Office 窗口仅在浏览器中用假数据验证过,未在真实应用中运行,未替换已安装应用 |

打包流程(每次都这样,别改顺序):

1. `node scripts/build-frontend.mjs`
2. `devstore run rust/current -- /usr/bin/env PATH="<nvm node>:$PATH" node node_modules/.bin/tauri build --config src-tauri/tauri.dist.conf.json --bundles app`(不要带 `tauri.localsign.conf.json`:`devstore run` 改了 `HOME`,看不到登录钥匙串,会报 "The specified item could not be found in the keychain")
3. 在 devstore 之外:`codesign --force --deep --options runtime --sign "NPi Desktop Local" "<.app>"`
4. `bundle_dmg.sh --skip-jenkins --volname "NPi Desktop" --app-drop-link 480 170 <输出> <含 .app 的目录>`(Tauri 自带的 dmg 步骤会被 Finder 拒绝)
5. `hdiutil verify`、挂载后 `codesign --verify --deep --strict`、`shasum -a 256`
6. 把 dmg 放进 `dist/<版本>/`,校验通过后删除 `target/release/bundle/` 下的源 dmg、`.app` 和 `rw.*.dmg`,只保留 `dist/` 里的那一份(增量编译缓存不动;`bundle/dmg/bundle_dmg.sh` 与 `icon.icns` 保留,只打 app 时 Tauri 不会重新生成它们,第 4 步要用)

内核：`omp/packages/coding-agent` 里 `bun run build`（本机 `.tools/node_modules/.bin/bun`），产物 `dist/npi` 复制到 `desktop/src-tauri/binaries/npi-aarch64-apple-darwin`。最近一次编译于 2026-11-08（含 Collapse 档位与颜文字等待行，内核版本仍为 `npi/2.0.10`）；对应 app 使用 DevStore `rust/current` 构建，只生成 app、dmg。

CLI 主题更新(2026-11-08，已重编译并打包至上表 `cli-themes` dmg)：启动 Logo、字标与流光改为读取当前主题的 `text` / `accent` / `borderAccent`，静止画面切换主题时同步刷新；新增内置 `dark-tokyo-neon`、`dark-tokyo-rain`、`dark-tokyo-chrome` 三款赛博朋克配色。17 项 welcome/tip/startup-splash 定向测试及 TUI 类型检查通过；终端渲染冒烟覆盖三款新主题与 light、动画/静止画面、全尺寸/紧凑 splash 和 256 色转换。三款配色同时导入本机 iTerm2 Color Presets，24 个颜色字段逐一与导出值一致，原有预设及全部 Profile 配置保持不变；导出文件保存在 `dist/3.0.0.138/iterm2/*.itermcolors`。未修改用户主题选择或已安装应用。

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
