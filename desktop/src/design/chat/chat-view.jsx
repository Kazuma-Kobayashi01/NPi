/* chat/chat-view.jsx — top-level chat surface. Sticks to the bottom while
   the user is already there; a manual scroll up unpins the view and shows
   a "Jump to latest" button instead of forcing the viewport back on every
   streamed update (issue #20). See app/scroll-pin.js for the pin state
   machine. Also routes each message to the right bubble component; the
   minimap-hover cross-highlight (mm-hot) flows through here via the
   `hoveredMsgIdx` prop. */

const { UserBubble: _CV_UserBubble, ToolCard: _CV_ToolCard, AssistantBubble: _CV_AssistantBubble, AskBubble: _CV_AskBubble, RetryRow: _CV_RetryRow, GoalRow: _CV_GoalRow, ApprovalRow: _CV_ApprovalRow, Icon: _CV_Icon, TOOL_META: _CV_TOOL_META, ToolGroup: _CV_ToolGroup, WorkBox: _CV_WorkBox, WorkflowPipeline: _CV_WorkflowPipeline } = window;
const { nextPinned: _CV_nextPinned, shouldRepin: _CV_shouldRepin, afterOutput: _CV_afterOutput } = window.OMP_SCROLL_PIN;
const { collapseStates: _CV_collapseStates, PREVIEW_LINES: _CV_PREVIEW_LINES } = window.OMP_COLLAPSE;
// Lines a collapsed block keeps in view, for chat.css; one object, so a
// streaming frame does not diff a fresh style.
const _CV_PAD_STYLE = { "--collapse-lines": _CV_PREVIEW_LINES };

// ── Per-bubble memo wrappers ────────────────────────────────────────────────
// Primary streaming-perf win: only the live tail (new object ref per
// message_update) re-renders; stable messages bail out automatically.
// Created once at module load — React.memo returns a stable component type.
const _CV_UserBubble_M      = React.memo(_CV_UserBubble);
const _CV_ToolCard_M        = React.memo(_CV_ToolCard);
const _CV_AssistantBubble_M = React.memo(_CV_AssistantBubble);
const _CV_AskBubble_M       = React.memo(_CV_AskBubble);

const _CV_ToolGroup_M      = React.memo(_CV_ToolGroup);
const _CV_WorkBox_M        = React.memo(_CV_WorkBox);

// Title and chip per row status (app/compaction.js); only "error" is a failure.
const _CV_COMPACT_TEXT = {
  pending:   ["compact.running", "compact.runningChip"],
  done:      ["compact.done", null],
  skipped:   ["compact.skipped", "compact.skippedChip"],
  cancelled: ["compact.cancelled", "compact.cancelledChip"],
  error:     ["compact.failed", "compact.failedChip"],
};

const CompactRow = React.memo(function CompactRow({ msg }) {
  const [open, setOpen] = React.useState(false);
  const t        = window.OMP_I18N.t;
  const status   = _CV_COMPACT_TEXT[msg.status] ? msg.status : "error";
  const pending  = status === "pending";
  const done     = status === "done";
  const error    = status === "error";
  const COLOR    = error ? "var(--rose)" : done || pending ? "var(--lilac)" : "var(--fg-3)";
  const [titleKey, chipKey] = _CV_COMPACT_TEXT[status];
  // Shared with the status bar / usage-stats panel (adapter.js). Gate on
  // the *source* value being non-null, not the formatted string —
  // `formatTokens(null)` returns the truthy "—" placeholder, which would
  // render an empty "— before" chip instead of hiding it.
  const tok      = window.formatTokens(msg.tokensBefore);
  const body     = done ? msg.summary : msg.reason;
  const hasBody  = !!body && !pending;

  return (
    <div className="row tool fade-up">
      <div className="ass-rail">
        <div className="tool-glyph" style={{ borderColor: COLOR, color: COLOR }}>
          <_CV_Icon name={error ? "warn" : "context"} size={11} color={COLOR} />
        </div>
        <div className="ass-thread" />
      </div>
      <div className={`tool-card ${pending ? "running" : "ok"}`}
        style={error ? { borderColor: "color-mix(in oklab, var(--rose) 35%, var(--line-bright))" } : {}}>
        <div className="tool-card-head"
          style={{ cursor: hasBody ? "pointer" : "default" }}
          onClick={() => hasBody && setOpen(o => !o)}>
          <span className="tool-tag" style={{
            color: COLOR,
            background: `color-mix(in oklab, ${COLOR} 14%, transparent)`,
            borderColor: `color-mix(in oklab, ${COLOR} 30%, var(--line))`,
          }}>compact</span>
          <span className="tool-title">
            {done ? (msg.shortSummary || t(titleKey)) : t(titleKey)}
          </span>
          <div className="tool-card-spacer" />
          {pending && (
            <span className="chip accent" style={{ animation: "pulseDot 1.4s infinite" }}>
              <span className="dot live" />{" "}{t(chipKey)}
            </span>
          )}
          {done && msg.tokensBefore != null && (
            <span className="chip muted mono">{t("compact.before", { n: tok })}</span>
          )}
          {!pending && chipKey && <span className="chip" style={{ color: COLOR }}>{t(chipKey)}</span>}
          {hasBody && <_CV_Icon name={open ? "chev" : "chevR"} size={10} color="var(--fg-4)" />}
        </div>
        {open && hasBody && (
          <div className="compact-body selectable">
            {body}
          </div>
        )}
      </div>
    </div>
  );
});

// A background job whose result omp just delivered (`async-result`,
// app/turn-status.js `finishedJobsOf`): the agent's next reply is what that
// result woke it for, so the job is named above it. Live-only, like tool
// cards — get_messages' merge keeps it in place.
const JobRow = React.memo(function JobRow({ msg, idx, highlighted }) {
  const { color, icon, label } = _CV_TOOL_META.job;
  return (
    <div className={`row tool fade-up${highlighted ? " mm-hot" : ""}`} data-msg-idx={idx}>
      <div className="ass-rail">
        <div className="tool-glyph" style={{ borderColor: color, color }}>
          <_CV_Icon name={icon} size={11} color={color} />
        </div>
        <div className="ass-thread" />
      </div>
      <div className="tool-card ok">
        <div className="tool-card-head">
          <span className="tool-tag" style={{
            color,
            background: `color-mix(in oklab, ${color} 14%, transparent)`,
            borderColor: `color-mix(in oklab, ${color} 30%, var(--line))`,
          }}>{label}</span>
          <span className="tool-title" title={msg.label ?? undefined}>
            {msg.jobId} finished
            {msg.label && <span className="job-label"> · {msg.label}</span>}
          </span>
          <div className="tool-card-spacer" />
          {msg.type && <span className="chip muted mono">{msg.type}</span>}
          {msg.durationMs != null && <span className="chip muted mono">{window.OMP_SUBAGENTS.fmtDuration(msg.durationMs)}</span>}
        </div>
      </div>
    </div>
  );
});

// Collapsible blocks (#44, #43): `foldModes` (`{ thinking, tool }`) and
// `collapseAfter` are the tweaks, `collapseOverrides` this tab's
// `_id → collapsed` toggles, and `onToggleCollapse(id, collapsed)` flips one
// (app/chat-collapse.js). `followOutput` and `submitKey` are tweaks too: the
// first lets new output move a view at the bottom, the second is the ask
// editor's send chord.
function ChatView({ messages, planMode, annotations, onAnnotate, hoveredMsgIdx, onAskAnswer, onAskDialogAnswer, onConfirmAsk, onCancelAsk, onGrantApproval, onStopRetry, onUiSubmit, onUiCancel, onUiRendered, uiDisabled = false, hasProjectPath, onInspectSubagent, onOpenFile, onPreviewFile, projectRoot, foldModes, collapseAfter, collapseOverrides, onToggleCollapse, followOutput = true, submitKey = "enter", wallpaper = null, onRewind = null, streaming = false, kanban = [], subagents = [], showWorkflowPipeline = true }) {
  const scrollRef   = React.useRef(null);
  const pinnedRef   = React.useRef(true);   // assume start pinned to bottom
  const prevTopRef  = React.useRef(0);
  const lastIdRef   = React.useRef(null);
  const renderedRef = React.useRef(false); // first transcript effect done
  const followRef   = React.useRef(followOutput);
  followRef.current = followOutput;
  // Mirrors pinnedRef into render so the "Jump to latest" button can show —
  // scroll position itself must not force a re-render on every wheel tick.
  const [showJump, setShowJump] = React.useState(false);
  // While the user reads history: the last message shown when they left the
  // bottom. Newer blocks don't auto-collapse older ones meanwhile, so the
  // text being read doesn't fold away under the reader.
  const [collapseHorizon, setCollapseHorizon] = React.useState(null);
  // Anchor for keeping reading position stable when blocks fold/extend above viewport
  const anchorRef = React.useRef(null);

  const updateAnchor = React.useCallback(() => {
    const el = scrollRef.current;
    if (!el || pinnedRef.current) {
      anchorRef.current = null;
      return;
    }
    const containerRect = el.getBoundingClientRect();
    const rows = el.querySelectorAll(".chat-pad > .row, .chat-pad > .tool-group, .chat-pad > .work-box");
    for (let i = 0; i < rows.length; i++) {
      const r = rows[i].getBoundingClientRect();
      if (r.bottom > containerRect.top) {
        anchorRef.current = { el: rows[i], offsetTop: r.top - containerRect.top };
        return;
      }
    }
    anchorRef.current = null;
  }, []);


  const setPinned = (val) => {
    pinnedRef.current = val;
    setShowJump(!val);
    setCollapseHorizon(val ? null : lastIdRef.current);
  };

  // Re-syncs prevTopRef after every programmatic scroll, or the next
  // nextPinned call would misread it as a user scroll-up.
  const stickToBottom = (el) => {
    el.scrollTop = el.scrollHeight;
    prevTopRef.current = el.scrollTop;
  };

  // Only a real user scroll — up and away from the bottom — unpins.
  // Scrolling back down near the bottom re-pins. See app/scroll-pin.js.
  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const pinned = _CV_nextPinned(pinnedRef.current, prevTopRef.current, el);
    prevTopRef.current = el.scrollTop;
    if (pinned !== pinnedRef.current) setPinned(pinned);
    updateAnchor();
  };

  const jumpToLatest = () => {
    const el = scrollRef.current;
    if (!el) return;
    stickToBottom(el);
    setPinned(true);
  };

  // Expanding output is a reading action: the next token must not pull the
  // just-opened records away. Keep the existing bottom-follow button.
  const handleToggleCollapse = React.useCallback((id, collapsed) => {
    if (collapsed) {
      pinnedRef.current = false;
      setShowJump(true);
      setCollapseHorizon(lastIdRef.current);
    }
    updateAnchor();
    onToggleCollapse(id, collapsed);
  }, [onToggleCollapse, updateAnchor]);

  // Auto-scroll only while pinned and following output. A new message
  // block (tool card, new assistant turn, …) during a run must NOT itself
  // force a jump — that was issue #20: every streamed update snapped the
  // view to the bottom even while the user was reading history. Sending a
  // new prompt explicitly re-pins, and so does the transcript being reset
  // to empty (/new, a profile switch respawning the tab, or the tab itself
  // being torn down) — none of those change `activeSessionId`, so the `key`
  // remount on tab switch (app-live.jsx) doesn't reach them, and a leftover
  // unpinned state would otherwise strand the button over an empty chat.
  // The first render (that remount) also opens on the latest line. With
  // `followOutput` off nothing else scrolls (app/scroll-pin.js afterOutput).
  React.useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const repin = !renderedRef.current || _CV_shouldRepin(lastIdRef.current, messages);
    renderedRef.current = true;
    lastIdRef.current = messages[messages.length - 1]?._id ?? null;
    const next = _CV_afterOutput({ pinned: pinnedRef.current, follow: followRef.current, repin }, el);
    if (next.pinned !== pinnedRef.current) setPinned(next.pinned);
    if (next.stick) stickToBottom(el);
  }, [messages]);

  // The composer below grows and shrinks on its own (the queue strip, image
  // attachments, a growing draft), which shortens this view from below
  // with no scroll event and no new message: keep a pinned view on the
  // latest line through that. `stickToBottom` reads refs only, so the
  // first render's copy stays valid.
  React.useEffect(() => {
    const el = scrollRef.current;
    if (!el) return undefined;
    const ro = new ResizeObserver(() => { if (pinnedRef.current) stickToBottom(el); });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  // Preserve reading anchor when elements above viewport fold/extend while unpinned
  React.useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || pinnedRef.current) return;
    const anchor = anchorRef.current;
    if (anchor && anchor.el && el.contains(anchor.el)) {
      const containerRect = el.getBoundingClientRect();
      const currentOffset = anchor.el.getBoundingClientRect().top - containerRect.top;
      const diff = currentOffset - anchor.offsetTop;
      if (Math.abs(diff) > 0.5) {
        el.scrollTop += diff;
        prevTopRef.current = el.scrollTop;
      }
    }
  });


  // Only the last completed assistant message is annotatable in plan mode
  let lastAsstIdx = -1;
  if (planMode) {
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].kind === "assistant" && !messages[i].streaming && !messages[i].superseded) { lastAsstIdx = i; break; }
    }
  }

  const collapsed = React.useMemo(() => _CV_collapseStates(messages, {
    ...foldModes, after: collapseAfter, overrides: collapseOverrides,
    horizonId: collapseHorizon, hidden: window.OMP_TURN_STATUS.isHidden,
  }), [messages, foldModes.thinking, foldModes.tool, collapseAfter, collapseOverrides, collapseHorizon]);

  // One folded box per run of thinking and tool calls, or each as its own row (foldModes.work).
  const rows = React.useMemo(() => {
    if (foldModes.work === "box" && window.OMP_WORK_BOX) {
      return window.OMP_WORK_BOX.groupWork(messages, { hidden: window.OMP_TURN_STATUS.isHidden });
    }
    if (window.OMP_TOOL_GROUPS?.groupMessages) {
      return window.OMP_TOOL_GROUPS.groupMessages(messages, { hidden: window.OMP_TURN_STATUS.isHidden });
    }
    return messages.map((m, i) => ({ type: "message", msg: m, idx: i }));
  }, [messages, foldModes.work]);

  return (
    <div className={`chat-wrap${wallpaper ? " has-wallpaper" : ""}`}>
      {wallpaper && <div className="wallpaper" style={{ "--wp": `url("${new URL(wallpaper, document.baseURI).href}")` }} aria-hidden="true" />}
      <div className="chat-scroll selectable" ref={scrollRef} onScroll={onScroll}>
        <div className="chat-pad" style={_CV_PAD_STYLE}>
          {_CV_WorkflowPipeline && (
            <_CV_WorkflowPipeline
              phases={kanban}
              subagents={subagents}
              streaming={streaming}
              enabled={showWorkflowPipeline}
            />
          )}
          {rows.map((row) => {
            if (row.type === "work") {
              return (
                <_CV_WorkBox_M
                  key={row.key}
                  group={row}
                  hoveredMsgIdx={hoveredMsgIdx}
                  collapseOverrides={collapseOverrides}
                  collapsedStates={collapsed}
                  onToggleCollapse={handleToggleCollapse}
                  onInspectSubagent={onInspectSubagent}
                  onOpenFile={onOpenFile}
                  onPreviewFile={onPreviewFile}
                  projectRoot={projectRoot}
                />
              );
            }
            if (row.type === "tool_group") {
              return (
                <_CV_ToolGroup_M
                  key={row.key}
                  group={row}
                  hoveredMsgIdx={hoveredMsgIdx}
                  foldModes={foldModes}
                  collapseOverrides={collapseOverrides}
                  collapsedStates={collapsed}
                  onToggleCollapse={handleToggleCollapse}
                  onInspectSubagent={onInspectSubagent}
                  onOpenFile={onOpenFile}
                  onPreviewFile={onPreviewFile}
                  projectRoot={projectRoot}
                />
              );
            }
            const { msg: m, idx: i } = row;
            const hl = hoveredMsgIdx === i;
            if (window.OMP_TURN_STATUS.isHidden(m)) return null;
            if (m.kind === "ui") return <window.GenerativeUiCard key={`ui:${m.ui_id}`} msg={m} onSubmit={onUiSubmit} onCancel={onUiCancel} onRendered={onUiRendered} disabled={uiDisabled} />;
            if (m.kind === "user")    return <_CV_UserBubble_M    key={m._id ?? i} idx={i} highlighted={hl} msg={m} onRewind={onRewind} dup={messages.slice(0, i).filter(x => x.kind === "user" && x.text === m.text).length} />;
            if (m.kind === "compact") return <CompactRow          key={m._id ?? i} msg={m} />;
            if (m.kind === "job")     return <JobRow              key={m._id ?? i} idx={i} highlighted={hl} msg={m} />;
            if (m.kind === "retry")   return <_CV_RetryRow        key={m._id ?? i} idx={i} highlighted={hl} msg={m} onStop={onStopRetry} />;
            if (m.kind === "goal")    return <_CV_GoalRow         key={m._id ?? i} idx={i} highlighted={hl} msg={m} />;
            if (m.kind === "approval") return <_CV_ApprovalRow    key={m._id ?? i} idx={i} highlighted={hl} msg={m} />;
            if (m.kind === "tool")    return <_CV_ToolCard_M      key={m._id ?? i} idx={i} highlighted={hl} msg={m} onInspectSubagent={onInspectSubagent}
              onOpenFile={onOpenFile} onPreviewFile={onPreviewFile} projectRoot={projectRoot} collapsed={collapsed[m._id]} onToggleCollapse={handleToggleCollapse} />;
            if (m.kind === "ask")     return <_CV_AskBubble_M     key={m._id ?? i} idx={i} highlighted={hl} msg={m} onAnswer={onAskAnswer} onAnswerDialog={onAskDialogAnswer} onConfirm={onConfirmAsk} onCancelAsk={onCancelAsk} onGrant={onGrantApproval} hasProjectPath={hasProjectPath} submitKey={submitKey} />;
            return <_CV_AssistantBubble_M key={m._id ?? i} idx={i} highlighted={hl} msg={m}
              annotable={i === lastAsstIdx} hideThought={!!row.thoughtInBox}
              annotations={annotations}
              onAnnotate={onAnnotate}
              thoughtCollapsed={collapsed[m._id]} onToggleCollapse={handleToggleCollapse} />;
          })}
          {(() => {
            const p = window.OMP_RUN_PHASE.phase(messages, streaming);
            return p ? (
              <div className="run-status" role="status" aria-live="polite">
                <svg className="run-hourglass" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                  <path d="M4 2h8M4 14h8M5 2c0 3 3 4 3 6s-3 3-3 6M11 2c0 3-3 4-3 6s3 3 3 6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
                <span>{p.label}…</span>
              </div>
            ) : null;
          })()}
          <div style={{ height: 24 }} />
        </div>
      </div>
      {showJump && (
        <button type="button" className="chat-jump-latest" onClick={jumpToLatest}>
          <_CV_Icon name="chev" size={12} />
          Jump to latest
        </button>
      )}
    </div>
  );
}

Object.assign(window, { ChatView });
