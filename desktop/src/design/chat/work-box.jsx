/* chat/work-box.jsx — one quiet, folded disclosure for a turn's thinking and
   tool calls (app/work-box.js). Folded it is a single line — "已思考 · 用了 3
   个工具", or while the turn runs a spinner with what is happening now; opened
   it lists the thinking and the usual tool cards, unchanged. Only the user
   opens it; failures, approvals and asks are never inside (app/work-box.js). */

const { ToolCard: _WB_ToolCard, ThoughtBody: _WB_ThoughtBody, Icon: _WB_Icon } = window;
const { summarize: _WB_summarize, isCollapsed: _WB_isCollapsed } = window.OMP_WORK_BOX;

const _WB_ToolCard_M = React.memo(_WB_ToolCard);

function WorkBox({
  group, hoveredMsgIdx, collapseOverrides, collapsedStates, onToggleCollapse,
  onInspectSubagent, onOpenFile, onPreviewFile, projectRoot,
}) {
  const collapsed = _WB_isCollapsed(group.key, collapseOverrides);
  const sum = _WB_summarize(group.items);
  const bodyId = React.useId();
  const headRef = React.useRef(null);
  const hot = hoveredMsgIdx != null && collapsed && group.indices.includes(hoveredMsgIdx);

  const onKeyDown = (e) => {
    if (e.key === "Escape" && !collapsed) {
      e.stopPropagation();
      onToggleCollapse(group.key, false);
      headRef.current?.focus();
    }
  };

  return (
    <div className={`row work-box${sum.live ? " is-live" : ""}${hot ? " mm-hot" : ""}`}
      data-msg-idx={group.firstIdx} onKeyDown={onKeyDown}>
      <button ref={headRef} type="button" className="work-head"
        aria-expanded={!collapsed} aria-controls={bodyId}
        title={window.OMP_I18N.t(collapsed ? "work.open" : "work.close")}
        onClick={() => onToggleCollapse(group.key, collapsed)}>
        {sum.live
          ? <span className="work-live-dot pulse-dot" aria-hidden="true" />
          : <_WB_Icon name="thinking" size={12} color="var(--lilac)" />}
        <span className="work-label selectable">
          {sum.live ? `${sum.liveLabel}…` : sum.label}
        </span>
        {sum.live && sum.tools > 0 && <span className="work-count">{window.OMP_I18N.t("work.tools", { n: sum.tools })}</span>}
        <span className="work-chev" aria-hidden="true"><_WB_Icon name="chevR" size={11} color="currentColor" /></span>
      </button>
      {!collapsed && (
        <div className="work-body" id={bodyId}>
          {group.items.map(({ kind, msg, idx }) => (kind === "thought"
            ? (
              <div className="work-thought" key={`t:${msg._id}`}>
                <div className="work-thought-label">{window.OMP_I18N.t("work.thoughtLabel")}</div>
                <_WB_ThoughtBody thought={msg.thought} />
              </div>
            ) : (
              <_WB_ToolCard_M key={msg._id} idx={idx} highlighted={hoveredMsgIdx === idx} msg={msg}
                onInspectSubagent={onInspectSubagent} onOpenFile={onOpenFile} onPreviewFile={onPreviewFile}
                projectRoot={projectRoot} collapsed={collapsedStates[msg._id]} onToggleCollapse={onToggleCollapse} />
            )))}
        </div>
      )}
    </div>
  );
}

Object.assign(window, { WorkBox });
