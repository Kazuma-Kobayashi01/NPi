# NPi Desktop Agent 入口

- 开始项目工作前，必须读取 [.agent/README.md](.agent/README.md) 中的项目级注意事项；当前总进度、打包流程与下一步以 [docs/PROGRESS.md](docs/PROGRESS.md) 为准。
- 修改桌面端时，同时遵守 [desktop/AGENTS.md](desktop/AGENTS.md)，并按其索引读取对应架构文档。
- 修改 Agent 内核时，同时遵守 [omp/AGENTS.md](omp/AGENTS.md)；`omp/` 是独立 Git 仓库。
- 用户指定：所有 Rust 操作必须使用 DevStore 管理的 `rust/current` 环境，不绕过 DevStore 另装或使用其他 Rust 环境。
- 本项目用于自举开发；先验证开发产物，不擅自替换正在使用的应用或内核，不影响当前会话与真实用户数据。
- 用户指定：每次构建仅生成 app 和 dmg（不生成 zip），统一将打包生成的 dmg 放置在根目录下的 dist/{version} 文件夹中（如 dist/1.2.0-client/）。打包完成并校验后，dmg 只保留在 dist/{version} 中，删除 `desktop/src-tauri/target/release/bundle/` 下的源 dmg、app 及 `rw.*.dmg` 等中间产物（不动 release 增量编译缓存，保留 `bundle/dmg/bundle_dmg.sh` 与 `icon.icns`）。
- 分支与验证铁律：每次进行修改、删除等操作前必须确认当前分支正确；严禁进行过度测试和反复验收等浪费资源收益小的操作。
- 用户指定：服务端适配全权写入 `omp`（NPi-Kernel）独立仓库中维护，不在本 Desktop 根仓库中更改；原 1.1.0-server 分支已停止支持。
- 用户指定：命名遵循“通用先行、平台派生”原则（Desktop 通用 `x.y.z.w` 四位，OS 适配 `x.y.z.w-os`；Kernel 命名为 `kernel.x.y.z-platform`，通用为 `universal`；微更新使用 `(CxxEPxx)` 形式标识）；通用版本领先专用版本属正常现象。

跨项目注意事项统一维护在 `.agent/`，具体实现规则维护在对应子项目，避免重复或相互漂移。
