# Academic 后续计划

> 总进度与下一步见 [`PROGRESS.md`](PROGRESS.md)。

承接 `docs/PLAN_3.0.md`(阶段 3、4)与 `docs/PLAN_LIBRARIAN.md`。状态写于 2026-11-05;"已做"指代码与单元测试完成,真机与真实模型的验证见末尾。

## 一、顺序推进的五项

| 项 | 内容 | 状态 |
|---|---|---|
| A. 阅读器(阶段 4) | 独立窗口:文献列表、章节、分页原文、全文搜索、`?anchor=` / `reader://goto` 从引用跳到原文页并高亮;未建索引时提示并可一键建立 | 已做(`librarian/reader.rs`、`design/librarian/reader-window.jsx`);入口:Academic 工具栏菜单"阅读器" |
| B. 摘要与对照表(阶段 3) | Schema v6:`summaries`(L1 章节 / L2 全文,记录 `source_hash`,文件变更即标 stale)、`artifacts`(对照表)。事实与单元格带引文则对原页核对:已匹配 / 未找到(未核实)/ 模型判断;`not_reported`、`not_comparable` 不填值。Copilot 工具:`research_summarize`、`research_summaries`、`research_compare`;阅读器"摘要"标签与"对照表" | 已做(`librarian/summaries.rs`) |
| C. 数值与表格核对 | 确定性核对:`research_check_number`(数字拼写归一:千分位、全角、负号、百分号;可要求单位与上下文)、`research_check_table`(Markdown 表按行列取格,数值比较,给出实际值) | 已做(同上) |
| D. 文献库窗口的搜索 | Librarian 窗口用项目 FTS5 索引排序(中文短词可搜),子串过滤作补充;无索引时仍是子串 | 已做(`library_search`) |
| E. 扫描版 PDF 的 OCR | macOS Vision 本机识别(`ocr.rs`),无文字层的页被识别并建块;状态 `ocr`、锚点 `locator_quality = ocr`,界面标"OCR"并提示核对;单文件最多 80 页 | 已做(仅 macOS) |

## 二、较小缺口(待做)

1. 手动合并重复文献。
2. OpenAlex 作为第二数据源(Crossref 之外)。
3. EndNote XML、`.nbib`(PubMed)、Web of Science 导入。
4. 把 PDF 拖到 Librarian 窗口。
5. 附件被移动后按哈希找回。
6. 没有 DOI 的 PDF:可选"让模型读第一页"补全元数据(须用户确认,不静默联网)。

## 三、推迟到 3.1

1. `research_propose_ops`:让 Copilot 提结构修改(授权绑定 `planId + baseRevision` 的预览、执行、日志、移入项目回收区)。
2. 按论文逐篇授权。

(从主 Chat 调用 Copilot 已提前做完:`copilot_ask`。)

## 四、尚未验证

1. PDF 规则(首页元数据)未对真实 PDF 逐一跑过;OCR 只在系统生成的单页扫描样本上验证过,中文、多栏、表格、倾斜页未验证。
2. Crossref 未对真实服务跑过。
3. Copilot 工具(含本轮新增的 5 个)只经单元测试:`set_host_tools` 握手与真实模型的使用方式未验证。
4. 阅读器、摘要标签、对照表界面只做了编译与静态样式检查,未在真实 app 里点过。
5. 阅读器不能从聊天里的引用直接点开(`OMP_OPEN_PANEL("reader", { anchor })` 已支持,聊天消息里还没有放这个入口)。
