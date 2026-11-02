/* ui/mermaid-view.jsx — Isolated Mermaid diagram renderer for assistant messages.
   Runs Mermaid in strict mode with securityLevel: 'strict' and renders
   the resulting SVG inside an <img> tag as a data URL to enforce total
   script/CSP isolation. Observes document theme class changes to re-render
   diagrams with actual resolved colors for light/dark themes. */

(function () {
  function useCurrentTheme() {
    const [themeSig, setThemeSig] = React.useState(() =>
      document.documentElement ? document.documentElement.className : ""
    );

    React.useEffect(() => {
      if (!document.documentElement) return;
      const observer = new MutationObserver(() => {
        setThemeSig(document.documentElement.className);
      });
      observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["class", "style"]
      });
      return () => observer.disconnect();
    }, []);

    return themeSig;
  }

  function configureMermaid() {
    if (!window.mermaid) return;
    const cs = window.getComputedStyle(document.documentElement);
    const isLight =
      document.documentElement.classList.contains("theme-latte") ||
      document.documentElement.classList.contains("theme-ivory");

    const bgElevated = cs.getPropertyValue("--bg-elevated").trim() || (isLight ? "#e6e9ef" : "#303446");
    const fg = cs.getPropertyValue("--fg").trim() || (isLight ? "#4c4f69" : "#cdd6f4");
    const fg2 = cs.getPropertyValue("--fg-2").trim() || (isLight ? "#6c6f85" : "#a6adc8");
    const line = cs.getPropertyValue("--line").trim() || (isLight ? "#ccd0da" : "#45475a");
    const accent = cs.getPropertyValue("--accent").trim() || (isLight ? "#1e66f5" : "#89b4fa");

    try {
      window.mermaid.initialize({
        startOnLoad: false,
        securityLevel: "strict",
        theme: isLight ? "default" : "dark",
        themeVariables: {
          darkMode: !isLight,
          background: bgElevated,
          primaryColor: bgElevated,
          primaryTextColor: fg,
          primaryBorderColor: line,
          lineColor: fg2,
          secondaryColor: line,
          tertiaryColor: bgElevated,
          fontFamily: "inherit"
        },
        suppressErrorRendering: true
      });
    } catch (_) {}
  }

  function normalizeMermaidSvg(rawSvg) {
    if (typeof DOMParser === "undefined" || typeof XMLSerializer === "undefined") return rawSvg;
    const doc = new DOMParser().parseFromString(rawSvg, "text/html");
    const svgEl = doc.querySelector("svg");
    if (!svgEl) return rawSvg;
    if (!svgEl.getAttribute("xmlns")) {
      svgEl.setAttribute("xmlns", "http://www.w3.org/2000/svg");
    }
    const vb = (svgEl.getAttribute("viewBox") || "").trim().split(/[\s,]+/).map(Number);
    if (vb.length === 4 && Number.isFinite(vb[2]) && vb[2] > 0 && Number.isFinite(vb[3]) && vb[3] > 0) {
      const wAttr = svgEl.getAttribute("width") || "";
      if (!wAttr || wAttr.endsWith("%")) svgEl.setAttribute("width", String(Math.ceil(vb[2])));
      const hAttr = svgEl.getAttribute("height") || "";
      if (!hAttr || hAttr.endsWith("%")) svgEl.setAttribute("height", String(Math.ceil(vb[3])));
    }
    return new XMLSerializer().serializeToString(svgEl);
  }

  function MermaidView({ code, isClosed }) {
    const themeSig = useCurrentTheme();
    const [svgUrl, setSvgUrl] = React.useState(null);
    const [error, setError] = React.useState(null);
    const [rendering, setRendering] = React.useState(false);
    const [copied, setCopied] = React.useState(false);

    React.useEffect(() => {
      if (isClosed === false) {
        setSvgUrl(null);
        setError(null);
        return;
      }
      const text = (code ?? "").trim();
      if (!text) {
        setSvgUrl(null);
        setError(null);
        return;
      }

      if (!window.mermaid) {
        setError(new Error("Mermaid library not loaded"));
        return;
      }

      configureMermaid();

      let active = true;
      setRendering(true);
      setError(null);

      const id = "mermaid-" + Math.random().toString(36).slice(2, 9);
      window.mermaid
        .render(id, text)
        .then((res) => {
          if (!active) return;
          const rawSvg = res?.svg ?? "";
          if (rawSvg) {
            const svg = normalizeMermaidSvg(rawSvg);
            const dataUrl = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
            setSvgUrl(dataUrl);
            setError(null);
          } else {
            setError(new Error("Empty SVG rendered"));
          }
        })
        .catch((err) => {
          if (!active) return;
          setError(err || new Error("Mermaid syntax error"));
        })
        .finally(() => {
          document.getElementById("d" + id)?.remove();
          document.getElementById(id)?.remove();
          if (active) setRendering(false);
        });

      return () => {
        active = false;
      };
    }, [code, isClosed, themeSig]);

    const handleCopy = React.useCallback(() => {
      if (!code) return;
      navigator.clipboard?.writeText(code).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1600);
      });
    }, [code]);

    if (isClosed === false) {
      return (
        <div className="mermaid-pending">
          <span className="pulse-dot" /> Streaming diagram…
        </div>
      );
    }

    if (rendering && !svgUrl) {
      return (
        <div className="mermaid-pending">
          <span className="pulse-dot" /> Rendering diagram…
        </div>
      );
    }

    if (error) {
      return (
        <div className="mermaid-error">
          <div className="mermaid-error-head">
            <span className="chip danger">Mermaid Error</span>
            <span className="mermaid-error-msg">{error.message}</span>
          </div>
          <pre className="code-block copy-host">
            <code className="hljs">{code}</code>
          </pre>
        </div>
      );
    }

    if (!svgUrl) {
      return null;
    }

    return (
      <div className="mermaid-card">
        <div className="mermaid-head">
          <button
            className="btn ghost is-floating"
            type="button"
            onClick={handleCopy}
            title="Copy Mermaid source"
            style={{ height: 20, fontSize: 11, padding: "0 6px" }}
          >
            {copied ? "copied!" : "copy source"}
          </button>
        </div>
        <div className="mermaid-img-wrap">
          <img
            src={svgUrl}
            alt="Mermaid diagram"
            className="mermaid-img"
            onError={() => {
              setSvgUrl(null);
              setError(new Error("Failed to decode rendered Mermaid SVG"));
            }}
          />
        </div>
      </div>
    );
  }

  window.MermaidView = MermaidView;
})();
