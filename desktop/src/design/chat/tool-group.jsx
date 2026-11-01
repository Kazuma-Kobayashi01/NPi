/* chat/tool-group.jsx — Compact expandable disclosure for consecutive
   completed tool calls. Keeps quiet linear styling, full accessibility,
   and stable key identity as new tools append. */

const { ToolCard: _TG_ToolCard, Icon: _TG_Icon } = window;
const { isGroupCollapsed: _TG_isGroupCollapsed, groupSummary: _TG_groupSummary } = window.OMP_TOOL_GROUPS;

const _TG_ToolCard_M = React.memo(_TG_ToolCard);

function ToolGroup({
  group,
  hoveredMsgIdx,
  foldModes,
  collapseOverrides,
  collapsedStates,
  onToggleCollapse,
  onInspectSubagent,
  onOpenFile,
  onPreviewFile,
  projectRoot,
}) {
  const tools = group.tools;
  const isSingle = tools.length === 1;
  const collapsed = isSingle ? false : _TG_isGroupCollapsed(group, { foldModes, collapseOverrides });
  const bodyId = React.useId();
  const headRef = React.useRef(null);

  // Minimap highlight: when folded (count >= 2), group is hot if any child matches hoveredMsgIdx.
  const isGroupHot = !isSingle && hoveredMsgIdx != null && group.indices.includes(hoveredMsgIdx);
  const highlighted = (!isSingle && collapsed) ? isGroupHot : false;

  const handleKeyDown = (e) => {
    if (!isSingle && e.key === "Escape" && !collapsed) {
      e.stopPropagation();
      onToggleCollapse(group.key, false);
      headRef.current.focus();
    }
  };

  const summary = isSingle ? "" : _TG_groupSummary(tools);

  return (
    <div
      className={`row tool-group${highlighted ? " mm-hot" : ""}`}
      data-msg-idx={group.firstIdx}
      onKeyDown={!isSingle ? handleKeyDown : undefined}
    >
      {!isSingle && (
        <button
          ref={headRef}
          type="button"
          className="tool-group-head"
          aria-expanded={!collapsed}
          aria-controls={bodyId}
          onClick={() => onToggleCollapse(group.key, collapsed)}
        >
          <span className="tool-group-count mono">{tools.length} tools</span>
          <span className="tool-group-status">· ok</span>
          <span className="tool-group-summary selectable">{summary}</span>
          <div style={{ flex: 1 }} />
          <_TG_Icon name={collapsed ? "chevR" : "chev"} size={10} color="var(--fg-4)" />
        </button>
      )}

      {(!collapsed || isSingle) && (
        <div className="tool-group-body" id={isSingle ? undefined : bodyId}>
          {group.items.map(({ msg, idx }) => {
            // For count 1: use normal collapsedStates[msg._id].
            // For expanded group (>= 2): all full ToolCard results by default,
            // individual manual toggles still respected.
            const cardCollapsed = isSingle
              ? collapsedStates[msg._id]
              : (Object.hasOwn(collapseOverrides, msg._id) ? collapseOverrides[msg._id] : false);

            return (
              <_TG_ToolCard_M
                key={msg._id}
                idx={idx}
                highlighted={hoveredMsgIdx === idx}
                msg={msg}
                onInspectSubagent={onInspectSubagent}
                onOpenFile={onOpenFile}
                onPreviewFile={onPreviewFile}
                projectRoot={projectRoot}
                collapsed={cardCollapsed}
                onToggleCollapse={onToggleCollapse}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}

Object.assign(window, { ToolGroup });
