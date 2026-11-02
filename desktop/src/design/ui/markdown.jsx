/* ui/markdown.jsx — Unified Markdown renderer with offline rich content support:
   KaTeX math formulas, native declarative charts, isolated Mermaid diagrams,
   and secure inline images connected to ImageLightbox.
   Uses single marked.parse with trusted placeholder slots + ReactDOM.createPortal. */

const { copyCodeBlockClick: _MdCopyClick } = window;

// Clicks in rendered markdown, delegated on its container: the code blocks'
// copy buttons, then links — `MarkdownLinkContext` carries a handler
// (app-live.jsx: the browser panel's localhost links), null for none.
const MarkdownLinkContext = React.createContext(null);

// Media context: one progressive image store per conversation
// (app-live.jsx → app/image-loader.js): { key, peekImage, subscribeImage,
// retryImage, resolveImage }. Images never fetch directly from the webview.
// Fail loudly if missing; no fake dummy fallbacks.
const MarkdownMediaContext = React.createContext(null);

function useMarkdownClick() {
  const onLink = React.useContext(MarkdownLinkContext);
  return React.useCallback(
    (e) => {
      _MdCopyClick(e);
      onLink?.(e);
    },
    [onLink]
  );
}

const escapeText = (text) =>
  String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

// ── Markdown renderer — uses marked + highlight.js when available ─────────
const MarkdownContent = ({ text, streaming }) => {
  const containerRef = React.useRef(null);
  const onClick = useMarkdownClick();
  const media = React.useContext(MarkdownMediaContext);
  const [slotItems, setSlotItems] = React.useState([]);
  const slotNodeMapRef = React.useRef(new Map());

  // Single standard marked.parse for all markdown (including math, lists, tables, links)
  const html = React.useMemo(() => {
    if (!text) return "";
    if (!window.marked) {
      return (
        "<p>" +
        escapeText(text)
          .replace(/\n\n/g, "</p><p>")
          .replace(/\n/g, "<br>") +
        "</p>"
      );
    }
    try {
      return window.marked.parse(text);
    } catch (_) {
      return "<pre>" + escapeText(text) + "</pre>";
    }
  }, [text]);

  // Synchronously discover placeholder slots after HTML DOM updates,
  // retaining existing DOM nodes when payload and state are unchanged
  // so portals do not unmount or re-render across streaming text appends.
  React.useLayoutEffect(() => {
    const nodes = containerRef.current.querySelectorAll(".md-slot");
    const items = [];
    const activeKeys = new Set();

    nodes.forEach((node, idx) => {
      const type = node.getAttribute("data-slot-type");
      let key = "";
      let slotData = { type };

      if (type === "chart") {
        const spec = node.getAttribute("data-spec") || "";
        const isClosed = node.getAttribute("data-closed") === "1";
        key = `chart:${spec}:${isClosed ? "1" : "0"}`;
        slotData = { type, spec, isClosed };
      } else if (type === "mermaid") {
        const code = node.getAttribute("data-code") || "";
        const isClosed = node.getAttribute("data-closed") === "1";
        key = `mermaid:${code}:${isClosed ? "1" : "0"}`;
        slotData = { type, code, isClosed };
      } else if (type === "image") {
        const src = node.getAttribute("data-src") || "";
        const alt = node.getAttribute("data-alt") || "";
        const title = node.getAttribute("data-title") || "";
        key = `image:${src}:${alt}:${title}`;
        slotData = { type, src, alt, title };
      } else {
        key = `unknown:${idx}`;
      }

      key = `${idx}:${key}`;
      activeKeys.add(key);

      let targetNode = node;
      if (slotNodeMapRef.current.has(key)) {
        const retainedNode = slotNodeMapRef.current.get(key);
        if (retainedNode && retainedNode !== node && node.parentNode) {
          node.parentNode.replaceChild(retainedNode, node);
          targetNode = retainedNode;
        }
      } else {
        slotNodeMapRef.current.set(key, node);
      }

      items.push({ id: key, node: targetNode, ...slotData });
    });

    // Clean up stale nodes no longer present in the document
    for (const k of slotNodeMapRef.current.keys()) {
      if (!activeKeys.has(k)) {
        slotNodeMapRef.current.delete(k);
      }
    }

    setSlotItems(items);
  }, [html]);

  // Late-bound references to components
  const ChartComp = window.ChartView;
  const MermaidComp = window.MermaidView;
  const ImageComp = window.MarkdownInlineImage;

  return (
    <div
      ref={containerRef}
      className={`md-content selectable${streaming ? " md-streaming" : ""}`}
      onClick={onClick}
    >
      <div className="md-html" dangerouslySetInnerHTML={{ __html: html }} />
      {slotItems.map((slot) => {
        let comp;
        if (slot.type === "chart") {
          comp = <ChartComp specText={slot.spec} isClosed={slot.isClosed} />;
        } else if (slot.type === "mermaid") {
          comp = <MermaidComp code={slot.code} isClosed={slot.isClosed} />;
        } else if (slot.type === "image") {
          comp = <ImageComp key={media.key} src={slot.src} alt={slot.alt} title={slot.title} />;
        }
        return window.ReactDOM.createPortal(comp, slot.node, slot.id);
      })}
    </div>
  );
};

Object.assign(window, {
  MarkdownContent,
  MarkdownLinkContext,
  MarkdownMediaContext,
  useMarkdownClick
});
