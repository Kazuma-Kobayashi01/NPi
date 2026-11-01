/* ═════════════════════════════════════════════════════════════════════
   design/workflow-pipeline.jsx — Minimal Floating Workflow Pipeline
   Presents a refined, lightweight stage capsule (e.g. 调研 ➔ 编辑 ➔ 编译 ➔ 验证).
   Downwards expands miniature parallel branch chips only when concurrent work occurs.
   Can be disabled globally via Settings.
   ═════════════════════════════════════════════════════════════════════ */

(function () {
  const { Icon } = window;

  const STAGE_RULES = [
    { label: "调研", match: /(research|investigat|explor|survey|analy|look|read|disc|调研|排查|分析|探索|收集|查阅)/i },
    { label: "方案", match: /(plan|design|architect|propos|方案|设计|规划|评估)/i },
    { label: "编辑", match: /(implement|edit|writ|cod|execut|fix|refactor|mod|实施|编码|编辑|开发|修改|重构|编写)/i },
    { label: "编译", match: /(build|compil|bundl|packag|构建|编译|打包)/i },
    { label: "验证", match: /(test|verif|validat|check|qa|smoke|测试|验证|冒烟|核验|自测|检查)/i },
    { label: "交付", match: /(deliver|releas|ship|cleanup|summar|commit|clean|交付|审查|归档|提交|总结|清理)/i },
  ];

  function toAbstractStageName(raw) {
    if (!raw) return "阶段";
    const found = STAGE_RULES.find(r => r.match.test(raw));
    if (found) return found.label;
    const clean = String(raw).replace(/^(Phase|Stage|步骤|阶段)\s*\d+[:：\s]*/i, "").trim();
    return clean.length <= 4 ? clean : clean.slice(0, 3) + "…";
  }

  function WorkflowPipeline({ phases, subagents, streaming, enabled = true }) {
    const [collapsed, setCollapsed] = React.useState(false);

    if (enabled === false) return null;

    const stages = React.useMemo(() => {
      if (Array.isArray(phases) && phases.length > 0) {
        return phases.map((p, idx) => {
          const tasks = p.items || p.tasks || [];
          const isDone = tasks.length > 0 && tasks.every(t => (typeof t === "object" ? t.status === "completed" || t.status === "done" : false));
          const isRunning = tasks.some(t => (typeof t === "object" ? t.status === "in_progress" || t.status === "running" : false));
          const runningBranches = tasks.filter(t => typeof t === "object" && (t.status === "in_progress" || t.status === "running"));

          return {
            id: p.name || `phase-${idx}`,
            label: toAbstractStageName(p.name || p.phase),
            status: isDone ? "done" : isRunning ? "running" : "pending",
            parallelTasks: runningBranches.map((t, ti) => {
              const full = t.content || t.name || `Branch ${ti + 1}`;
              return {
                id: t.id || `${idx}-${ti}`,
                label: full.length > 12 ? full.slice(0, 11) + "…" : full,
                status: "running"
              };
            })
          };
        });
      }

      if (Array.isArray(subagents) && subagents.length > 0) {
        const isRunning = subagents.some(s => s.status === "running");
        return [
          {
            id: "subagents",
            label: "并行任务",
            status: isRunning ? "running" : "done",
            parallelTasks: subagents.filter(s => s.status === "running").map(s => {
              const name = s.agent || s.task || "Agent";
              return {
                id: s.id,
                label: name.length > 12 ? name.slice(0, 11) + "…" : name,
                status: "running"
              };
            })
          }
        ];
      }

      return null;
    }, [phases, subagents]);

    if (!stages || stages.length === 0) return null;

    return (
      <div className="wf-pipeline-root" style={{
        position: "sticky", top: 6, zIndex: 9, margin: "0 auto 10px",
        width: "fit-content", maxWidth: "94%", display: "flex", flexDirection: "column",
        alignItems: "center"
      }}>
        {/* Main Minimal Capsule */}
        <div className="wf-capsule" style={{
          display: "inline-flex", alignItems: "center", gap: 6,
          padding: "3px 10px", borderRadius: 16,
          background: "color-mix(in oklab, var(--bg-elevated) 82%, var(--bg-window))",
          backdropFilter: "blur(10px)", WebkitBackdropFilter: "blur(10px)",
          border: "1px solid var(--line-soft, var(--line))",
          boxShadow: "0 2px 8px rgba(0,0,0,0.12)",
          fontSize: "var(--d-text-xs)", userSelect: "none"
        }}>
          {collapsed ? (
            <div style={{ display: "inline-flex", alignItems: "center", gap: 6, color: "var(--fg-3)", cursor: "pointer" }}
                 onClick={() => setCollapsed(false)}>
              <span className="pulse-dot" style={{ width: 6, height: 6 }} />
              <span style={{ fontWeight: 500 }}>工作流</span>
              <Icon name="chev" size={10} color="var(--fg-4)" />
            </div>
          ) : (
            <>
              {stages.map((st, i) => (
                <React.Fragment key={st.id}>
                  <div className={`wf-stage ${st.status}`} style={{
                    display: "inline-flex", alignItems: "center", gap: 4,
                    color: st.status === "running" ? "var(--accent)" : st.status === "done" ? "var(--fg-3)" : "var(--fg-4)",
                    fontWeight: st.status === "running" ? 600 : 400
                  }}>
                    <span style={{ fontSize: 10, display: "inline-flex", alignItems: "center" }}>
                      {st.status === "running" ? (
                        <span className="pulse-dot" style={{ width: 5, height: 5 }} />
                      ) : st.status === "done" ? (
                        <span style={{ color: "var(--lime, #40A02B)" }}>✓</span>
                      ) : (
                        "○"
                      )}
                    </span>
                    <span>{st.label}</span>
                    {st.parallelTasks.length > 1 && (
                      <span className="chip muted mono" style={{ fontSize: 9, padding: "0 3px", height: 14, lineHeight: "14px" }}>
                        {st.parallelTasks.length}并
                      </span>
                    )}
                  </div>
                  {i < stages.length - 1 && (
                    <span style={{ color: "var(--line)", display: "inline-flex", alignItems: "center" }}>➔</span>
                  )}
                </React.Fragment>
              ))}
              <button
                type="button"
                className="btn ghost icon"
                onClick={() => setCollapsed(true)}
                title="折叠工作流流水线"
                style={{ width: 16, height: 16, padding: 0, marginLeft: 2, color: "var(--fg-4)" }}
              >
                <Icon name="chevR" size={9} />
              </button>
            </>
          )}
        </div>

        {/* Parallel Branches Downward Expansion */}
        {!collapsed && stages.some(st => st.parallelTasks.length > 0) && (
          <div className="wf-parallel-rail" style={{
            display: "flex", gap: 6, marginTop: 4, alignItems: "center"
          }}>
            {stages.filter(st => st.parallelTasks.length > 0).flatMap(st => st.parallelTasks).map(task => (
              <div key={task.id} style={{
                display: "inline-flex", alignItems: "center", gap: 4,
                padding: "2px 7px", borderRadius: 10, fontSize: 10,
                background: "color-mix(in oklab, var(--bg-surface) 90%, transparent)",
                border: "1px solid var(--line-soft, var(--line))",
                color: "var(--fg-2)"
              }}>
                <span className="pulse-dot" style={{ width: 4, height: 4 }} />
                <span>{task.label}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }

  window.WorkflowPipeline = WorkflowPipeline;
})();
