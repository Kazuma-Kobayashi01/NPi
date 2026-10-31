/* chat/user-bubble.jsx — a user message: a rounded card, left-aligned text,
   and under it, at the right, the time and two small actions: Copy, and
   Rewind (go back to before this message and put its text in the prompt box).
   `dup` is how many earlier messages carry the same text: it picks the right
   entry when one prompt was sent twice. `onRewind` is null when it is not
   possible (a turn is running). */
const { ImageThumbs: _UB_ImageThumbs, Icon: _UB_Icon } = window;

function UserBubble({ msg, idx, highlighted, dup = 0, onRewind }) {
  const [copied, setCopied] = React.useState(false);
  const copy = async () => {
    if (await window.copyText(msg.text ?? "")) { setCopied(true); setTimeout(() => setCopied(false), 1200); }
  };
  return (
    <div className={`row user fade-up${highlighted ? " mm-hot" : ""}`} data-msg-idx={idx}>
      <div className="user-bubble selectable">
        <_UB_ImageThumbs images={msg.images} className="user-images" alt="attached image" />
        {msg.text && <div className="user-text">{msg.text}</div>}
      </div>
      <div className="user-foot">
        {msg.time && <span className="user-time">{msg.time}</span>}
        <button type="button" className="user-act" title={copied ? "Copied" : "Copy"} aria-label="Copy message" onClick={copy}>
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <rect x="5.5" y="5.5" width="8" height="8" rx="1.8" stroke="currentColor" strokeWidth="1.3" />
            <path d="M10.5 3.5v-.2A1.8 1.8 0 0 0 8.7 1.5H3.8A1.8 1.8 0 0 0 2 3.3v4.9a1.8 1.8 0 0 0 1.8 1.8h.2" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
          </svg>
        </button>
        {onRewind && (
          <button type="button" className="user-act" title="Rewind to here" aria-label="Rewind to this message" onClick={() => onRewind(msg, dup)}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path d="M2.5 8a5.5 5.5 0 1 0 1.8-4.1" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
              <path d="M2.2 1.8v3.4h3.4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}

Object.assign(window, { UserBubble });
