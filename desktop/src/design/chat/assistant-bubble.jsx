/* chat/assistant-bubble.jsx — assistant block (text + plan + thoughts +
   a failed request's failure). The plan block is chat/inline-plan.jsx
   (`InlinePlan`, which loads first). */

const _AB_substantial = text => (window.OMP_WORK_BOX ? window.OMP_WORK_BOX.hasSubstance(text) : !!text?.trim());
const { Icon: _ChatIcon, MarkdownContent: _ChatMd, AnnotablePlan: _ChatAP, CopyButton: _ChatCopy, ReplyFeedback: _ChatFeedback } = window;

// The message's raw markdown, one text block after another — what the
// model wrote, code fences and line breaks included. A live message can
// carry an empty text block (a turn that went straight to a tool call).
const _messageText = (msg) => (msg.blocks ?? [])
  .map((b) => {
    if (b.type === "text" && _AB_substantial(b.text)) return b.text;
    if (b.type === "image") return b.alt ? `[image: ${b.alt}]` : (b.mimeType ? `[image: ${b.mimeType}]` : "[image]");
    return null;
  })
  .filter(Boolean)
  .join("\n\n");

// A request that failed for good — omp's own retries are behind it
// (app/turn-status.js `failureOf`). `retryable` (the "temporary" chip) is
// omp's live `prompt_result` verdict and unknown for a reloaded turn.
function _AB_Failure({ failure: f }) {
  const where = [f.provider, f.model].filter(Boolean).join(" · ");
  return (
    <div className="ass-failure">
      <div className="ass-failure-head">
        <span className="ass-failure-title">request failed</span>
        {f.httpStatus != null && <span className="chip danger mono">HTTP {f.httpStatus}</span>}
        {f.retryable && (
          <span className="chip warn" title="omp already retried it; sending again later may work">temporary</span>
        )}
        {where && <span className="chip muted mono">{where}</span>}
      </div>
      <div className="ass-failure-msg selectable">{f.headline}</div>
      {f.raw !== f.headline && (
        <details className="ass-failure-raw">
          <summary>provider response</summary>
          <pre className="selectable">{f.raw}</pre>
        </details>
      )}
    </div>
  );
}

// How omp's automatic retries ended on this reply (app/turn-status.js
// `retryFinished`); the failed attempts left the conversation.
function _AB_Retries({ retries: r }) {
  const n = `${r.failed} failed attempt${r.failed === 1 ? "" : "s"}`;
  const [label, title] = r.outcome === "recovered"
    ? [`after ${n}`, "omp retried the request by itself until it answered"]
    : r.outcome === "stopped" ? [n, "Retrying was stopped"]
    : [n, "omp gave up retrying"];
  return <span className={`chip mono ${r.outcome === "recovered" ? "warn" : "muted"}`} style={{ color: r.outcome === "recovered" ? undefined : "var(--fg-3)" }} title={title}>{label}</span>;
}

const ThoughtBody = React.memo(function ThoughtBody({ id, thought }) {
  return (
    <div id={id} className="thought-body selectable">
      {thought}
    </div>
  );
});

function ThoughtDisclosure({ msg, collapsed, onToggleCollapse }) {
  const bodyId = React.useId();
  if (!msg.thought?.trim()) return null;

  const isActive = window.OMP_COLLAPSE.thinkingActive(msg);
  const isCollapsed = collapsed ?? !isActive;

  const durationText = typeof msg.thinkingDurationMs === "number"
    ? (msg.thinkingDurationMs < 1000 ? `${Math.round(msg.thinkingDurationMs)}ms` : `${(msg.thinkingDurationMs / 1000).toFixed(1)}s`)
    : null;

  return (
    <div className={`thought${isCollapsed ? " is-collapsed" : ""}`}>
      <button
        type="button"
        className="thought-disclosure"
        aria-expanded={!isCollapsed}
        aria-controls={bodyId}
        onClick={() => onToggleCollapse?.(msg._id, isCollapsed)}
        title={isCollapsed ? "展开思考过程" : "折叠思考过程"}
      >
        <_ChatIcon name={isCollapsed ? "chevR" : "chev"} size={11} color="var(--fg-3)" />
        <span className="thought-label" style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
          <_ChatIcon name="thinking" size={12} color={isActive ? "var(--lilac)" : "var(--fg-3)"} />
          <span>{isActive ? "思考中" : "已思考"}</span>
        </span>
        {durationText && <span className="thought-duration">{durationText}</span>}
      </button>
      {!isCollapsed && <ThoughtBody id={bodyId} thought={msg.thought} />}
    </div>
  );
}

// ChatView owns collapse defaults and persistent per-tab manual overrides.
// `hideThought`: the thinking sits in the turn's work box above (chat/work-box.jsx).
// `conversation` (app/reply-feedback.js `conversationKey`), `latestReply` and
// `onReplyAction` (app-live.jsx `handleReplyAction`) drive the 👍/👎 footer.
function AssistantBubble({ msg, idx, highlighted, annotable, annotations, onAnnotate, thoughtCollapsed, onToggleCollapse, hideThought = false, conversation = null, latestReply = false, onReplyAction = null, onOpenPlan = null }) {
  // A failed request has no text of its own; its copy is the provider's error.
  const copyText = msg.streaming ? "" : (_messageText(msg) || msg.failure?.raw || "");
  return (
    <div className={`row assistant fade-up${highlighted ? " mm-hot" : ""}`} data-msg-idx={idx}>
      <div className="ass-rail">
        <div className={`ass-glyph${msg.failure ? " failed" : ""}`}>
          <_ChatIcon name="sparkle" size={11} color={msg.failure ? "var(--rose)" : "var(--accent)"} />
        </div>
        <div className="ass-thread" />
      </div>
      <div className="ass-body">
        <div className="ass-meta">
          <span className="mono" style={{ color: "var(--accent)" }}>{msg.model ?? "–"}</span>
          <span className="chip muted" style={{ color: "var(--fg-3)" }}>{msg.time}</span>
          {msg.retries && <_AB_Retries retries={msg.retries} />}
        </div>
        {!hideThought && (
          <ThoughtDisclosure
            msg={msg}
            collapsed={thoughtCollapsed}
            onToggleCollapse={onToggleCollapse}
          />
        )}
        {msg.blocks?.map((b, i) => {
          // A lone "." between tool calls is filler, not shown (app/work-box.js).
          if (b.type === "text" && !msg.streaming && !_AB_substantial(b.text)) return null;
          if (b.type === "text") {
            // Last message in plan mode: render annotatable blocks (not streaming)
            if (annotable && !msg.streaming) {
              return (
                <_ChatAP key={i} text={b.text}
                  annotations={annotations}
                  onAnnotate={onAnnotate} />
              );
            }
            return (
              <_ChatMd key={i} text={b.text}
                streaming={msg.streaming && i === msg.blocks.length - 1} />
            );
          }
          if (b.type === "plan") {
            return <InlinePlan key={i} plan={b} onOpenPlan={onOpenPlan} />;
          }
          if (b.type === "image") return <window.AssistantImage key={i} image={b} />;
          return null;
        })}
        {msg.failure && <_AB_Failure failure={msg.failure} />}
        {copyText && (
          <div className="ass-actions">
            <_ChatCopy className="ass-copy" label="Copy message" text={copyText} />
            {conversation && onReplyAction && !msg.streaming && !msg.failure && Number.isFinite(msg.ts) && (
              <_ChatFeedback conversation={conversation} ts={msg.ts}
                excerpt={latestReply ? "" : copyText.slice(0, 600)} onSend={onReplyAction} />
            )}
          </div>
        )}
      </div>
    </div>
  );
}

Object.assign(window, { AssistantBubble, ThoughtDisclosure, ThoughtBody });
