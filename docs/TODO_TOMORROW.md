# 明日清单(2026-11-05)

> 已并入 `docs/PROGRESS.md`,以那里为准。状态:第 1 项(清缓存)**已做**;第 2 项(Chat 首字)除"降思考档位(用户否决)"与"埋点"外**已做**;第 3 项(评估 Chromium)**未做**;第 4 项(二级页面独立窗口)**部分做**。


## 1. 第一项：清理所有构建缓存（用户已定，明天一早先做）
- 候选：`desktop/src-tauri/target/debug`（约 32G）、`release/bundle/macos/rw.*.dmg`、已合并的 agent worktree `.claude/worktrees/agent-a62e19c25e045f77c` 及分支、旧 `/private/tmp/claude-501` 临时目录。
- 保留：`omp/target`、cargo registry、node_modules。删除前先列出确切路径请用户确认。

## 2. Chat 首字慢（来自排查报告）
1. Chat 默认思考档位降到 low/medium（Chat 的 `--config` 覆盖里设 `defaultThinkingLevel`）。
2. 删掉 `desktop/src-tauri/src/app_mode.rs` `CHAT_PROMPT` 里"主动联网 / 主动配图 / 主动出表单"的措辞，改为需要时才用。
3. Chat 压缩阈值 `thresholdPercent` 70 → 85（`app_mode.rs` `CHAT_COMPACTION_OVERLAY`）。
4. 裁剪 Chat 的工具定义（固定请求约 11.5k token，大头是工具）。
5. Desktop 加首字耗时埋点（发送 → 首个 text_delta），改前改后对比。

## 3. 评估改用 Chromium
- 已知：Tauri 2.12 在 macOS 用系统 WKWebView；CEF 方案需要 Tauri 3 alpha（`tauri-runtime-cef`），此前用户已否决直接切换。
- 明天只做评估：改造范围、体积（chrome-headless-shell 已占 204MB）、风险，给出修改方案，不动代码。

## 4. 二级页面独立窗口（本轮已做第一批，余下待做）
- 已做（未编译验证）：Librarian、用量统计改为独立 OS 窗口。
- 待做：Settings、Git 浏览器、Plan、审批规则、历史会话。它们依赖主窗口的实时状态（当前标签页、会话、`bridge`），需要先做跨窗口通信（事件 / 命令）。
