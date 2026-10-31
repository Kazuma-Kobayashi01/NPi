# NPi Desktop Agent 入口

- 开始项目工作前，必须读取 [.agent/README.md](.agent/README.md) 中的项目级注意事项。
- 修改桌面端时，同时遵守 [desktop/AGENTS.md](desktop/AGENTS.md)，并按其索引读取对应架构文档。
- 修改 Agent 内核时，同时遵守 [omp/AGENTS.md](omp/AGENTS.md)；`omp/` 是独立 Git 仓库。
- 用户指定：所有 Rust 操作必须使用 DevStore 管理的 `rust/current` 环境，不绕过 DevStore 另装或使用其他 Rust 环境。
- 本项目用于自举开发；先验证开发产物，不擅自替换正在使用的应用或内核，不影响当前会话与真实用户数据。

跨项目注意事项统一维护在 `.agent/`，具体实现规则维护在对应子项目，避免重复或相互漂移。
