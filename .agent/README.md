# NPi Desktop 项目级注意事项

本目录保存本项目的 Agent 开发约定，不存放凭据、会话数据或构建产物。以下路径除链接外均相对项目根目录。

## 开始工作前

- 本项目是用户正在使用的 Agent 源码；自举开发不得影响当前会话和已安装应用。
- 修改 `desktop/` 前读取 [桌面端约定](../desktop/AGENTS.md)，再按其中索引读取对应的架构文档。
- 修改 `omp/` 前读取 [内核约定](../omp/AGENTS.md)；本文件补充跨项目约束，不替代子目录规则。
- [Academic 计划](../docs/PLAN.md) 是未来规划，不代表功能已实现，也不代表用户授权本次实施。

## 代码与仓库边界

- `desktop/`：Tauri 2 + Rust 桌面端，React 19 普通脚本前端，无 bundler。
- `omp/`：独立嵌套 Git 仓库，根仓库将其忽略；两边的修改、提交、推送需分别处理，不擅自提交或推送。
- Agent 默认实现入口是 `omp/packages/coding-agent/`；模型客户端、运行时和模型目录分别在 `packages/ai/`、`packages/agent/`、`packages/catalog/`。
- 桌面调用链：React → `desktop/src/live.js` → `desktop/src-tauri/src/agent/` → 每标签页独立的 `npi --mode rpc-ui` 进程。
- 核实当前分支、工作区与实际代码，不把历史记录中的版本或环境状态当作当前事实。

## Rust 必须使用 DevStore

- 用户明确要求：所有 Rust 开发、构建、测试和工具链操作使用 DevStore 管理的 `rust/current` 环境，包括 Tauri 及 omp 原生模块。
- 使用前读取 DevStore skill，检查 `devstore status`，必要时检查 `devstore inspect rust/current`；MCP 可用时优先使用对应工具。
- 命令通过 `devstore run rust/current -- <命令>` 或 MCP `devstore_run` 执行；也可使用 DevStore 提供的完整会话变量，确保 `PATH`、`CARGO_HOME`、`RUSTUP_HOME` 均来自该环境。
- 不只看 `cargo` 是否存在；先确认环境挂载状态与实际工具链来源。工具链版本仍遵守各子项目配置。
- 不用 Homebrew 或独立 rustup 安装替代工具链，不重建 `~/.cargo` / `~/.rustup`，不修改 shell 配置中的 DevStore 托管块。
- 涉及需要批准的 DevStore 计划，交用户批准；不得代批、强制卸载或自行删除、重建环境。

## 实现时的关键约束

- UI 权威源码是 `desktop/src/design/`；禁止从 `desktop/design/` 原型目录覆盖，以免丢失桥接逻辑。
- `desktop/src/index.html` 的脚本顺序就是依赖图；新增或拆分脚本必须维护加载顺序及 IIFE / 全局命名约定。
- 保持离线资源和严格 CSP；不引入 CDN 或为绕过错误而放宽安全策略。
- RPC、IPC、子进程及资源清理复用现有桥接、校验和生命周期机制；具体不变量以桌面端约定及架构文档为准。
- omp 的模型策略、提示词、生成文件等沿用其既有约定，不另建硬编码或重复工具函数。

## 安全自举与验证

- 先构建和验证开发产物，再按用户要求安装或替换；不要覆盖当前使用的应用、随包内核或用户数据来试错。
- 不终止当前 Agent 进程，不擅自变更登录凭据、审批策略或真实会话。`devstore run` 仅选择环境，不是隔离沙箱。
- RPC 探测优先使用临时 HOME、临时 `PI_CODING_AGENT_DIR`、临时工作目录和 `--no-session`；清除继承的 profile / 凭据，禁止读取真实登录数据。
- 只测启动时使用 `--mode rpc-ui`，显式选择模型后发 `get_state`，不发送 `prompt`；握手成功不等于模型调用已验证。
- 注意 `desktop/tests/test-rpc.mjs` 默认会发送模型请求，不把它当作无请求的启动探针。
- 前端按 `desktop/package.json` 的脚本验证；Rust 按 `desktop/AGENTS.md` 的 fmt、test、clippy 要求在 DevStore 环境执行。
- omp 测试按其约定执行；原生 Rust 测试通过 DevStore 环境中的 `bun run test:rs`，不要直接改用 `cargo test`。
- 非平凡行为修改还需运行实际改动路径；UI 验证实际界面。仅文档修改检查内容和链接即可，不声称已验证应用行为。
- Node / npm、Bun 等工具使用前检查可用性；发现缺失先定位已有安装，不擅自重装或改写全局环境。

## 文档与发布

- 功能修改同步维护对应架构文档和 changelog；构建步骤参见 [项目 README](../README.md)。
- 发版时同步 `desktop/package.json`、`desktop/src-tauri/Cargo.toml`、`desktop/src-tauri/tauri.conf.json` 和 `omp/packages/utils/src/dirs.ts` 的 `NPI_VERSION`。
- omp 包自身的版本表示上游内核版本，不要为了同步 NPi 产品版本而批量替换。
- 此处只保留稳定约束；临时排查输出、个人路径、密钥和缓存不要写入项目规则。
