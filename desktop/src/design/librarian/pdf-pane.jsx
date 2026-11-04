/* librarian/pdf-pane.jsx — the reader's PDF view: the paper's own pages (PDF.js, vendor/pdfjs) with a
   selectable text layer. The bytes come from the project's `papers/` through `research_pdf` (the
   webview names the paper by id); pages render as they scroll into view and are released when far away.
   `page` + `quote` (with a changing `nonce`) take the view to a page and mark the quote on it, which is
   how a search hit or a citation's anchor shows its evidence. */

(function () {
  let loading = null;
  /** PDF.js, loaded once by vendor/pdfjs/loader.mjs. */
  function loadPdfjs() {
    if (window.OMP_PDFJS) return Promise.resolve(window.OMP_PDFJS);
    if (!loading) {
      loading = new Promise((resolve, reject) => {
        window.addEventListener("omp:pdfjs", () => resolve(window.OMP_PDFJS), { once: true });
        const tag = document.createElement("script");
        tag.type = "module";
        tag.src = "vendor/pdfjs/loader.mjs";
        tag.onerror = () => { loading = null; reject(new Error("could not load the PDF viewer")); };
        document.head.appendChild(tag);
      });
    }
    return loading;
  }

  const fold = c => !/[\s­\-‐‑–—]/.test(c);

  /** Marks the text-layer spans that hold `quote` (case, spacing and hyphens ignored); true when found. */
  function markQuote(layer, quote) {
    layer.querySelectorAll(".rd-pdf-mark").forEach(el => el.classList.remove("rd-pdf-mark"));
    const want = [...quote.toLowerCase()].filter(fold).join("").slice(0, 140);
    if (want.length < 6) return false;
    const spans = [...layer.querySelectorAll("span:not(.markedContent)")].filter(s => s.textContent);
    let flat = "";
    const owner = [];
    spans.forEach((s, i) => {
      for (const c of s.textContent.toLowerCase()) if (fold(c)) { flat += c; owner.push(i); }
    });
    let at = flat.indexOf(want);
    if (at < 0) at = flat.indexOf(want.slice(0, Math.max(20, want.length >> 1)));
    if (at < 0) return false;
    const from = owner[at], to = owner[Math.min(at + want.length, owner.length) - 1];
    for (let i = from; i <= to; i++) spans[i].classList.add("rd-pdf-mark");
    return true;
  }

  function PdfPage({ doc, num, scale, base, root, mark, onMarked }) {
    const holder = React.useRef(null);
    const canvas = React.useRef(null);
    const textDiv = React.useRef(null);
    const [on, setOn] = React.useState(false);
    const [size, setSize] = React.useState({ w: base.w * scale, h: base.h * scale });

    React.useEffect(() => {
      const el = holder.current;
      if (!el || !root) return undefined;
      const io = new IntersectionObserver(([e]) => setOn(e.isIntersecting), { root, rootMargin: "900px 0px" });
      io.observe(el);
      return () => io.disconnect();
    }, [root]);

    React.useEffect(() => {
      let task = null, dead = false, textLayer = null;
      const cv = canvas.current, td = textDiv.current;
      if (!on) {
        if (cv) { cv.width = 0; cv.height = 0; }
        if (td) td.replaceChildren();
        setSize({ w: base.w * scale, h: base.h * scale });
        return undefined;
      }
      (async () => {
        const pdfjs = await loadPdfjs();
        const page = await doc.getPage(num);
        if (dead) return;
        const vp = page.getViewport({ scale });
        const dpr = window.devicePixelRatio || 1;
        cv.width = Math.floor(vp.width * dpr); cv.height = Math.floor(vp.height * dpr);
        cv.style.width = `${vp.width}px`; cv.style.height = `${vp.height}px`;
        setSize({ w: vp.width, h: vp.height });
        task = page.render({ canvasContext: cv.getContext("2d"), viewport: vp, transform: dpr === 1 ? null : [dpr, 0, 0, dpr, 0, 0] });
        try { await task.promise; } catch { return; }
        if (dead) return;
        td.replaceChildren();
        td.style.setProperty("--scale-factor", String(vp.scale));
        textLayer = new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: td, viewport: vp });
        try { await textLayer.render(); } catch { return; }
        if (!dead && mark) onMarked(num, markQuote(td, mark.quote), td);
      })().catch(() => {});
      return () => { dead = true; try { task?.cancel(); } catch { /* done */ } try { textLayer?.cancel(); } catch { /* done */ } };
    }, [on, doc, num, scale, mark?.nonce]);   // eslint-disable-line react-hooks/exhaustive-deps

    return (
      <div className="rd-pdf-page" data-page={num} ref={holder} style={{ width: size.w, height: size.h, "--scale-factor": scale }}>
        <canvas ref={canvas} />
        <div className="rd-pdf-text textLayer" ref={textDiv} />
      </div>
    );
  }

  function PdfPane({ bridge, project, paperId, page, quote, nonce, onPage }) {
    const T = window.OMP_I18N.t;
    const [doc, setDoc] = React.useState(null);
    const [error, setError] = React.useState("");
    const [base, setBase] = React.useState(null);
    const [zoom, setZoom] = React.useState(null);       // null = fit the width
    const [fit, setFit] = React.useState(1);
    const [now, setNow] = React.useState(1);
    const [note, setNote] = React.useState("");
    const scroller = React.useRef(null);
    const [root, setRoot] = React.useState(null);
    const setScroller = React.useCallback(el => { scroller.current = el; setRoot(el); }, []);
    const scale = zoom ?? fit;

    React.useEffect(() => {
      let dead = false, loaded = null;
      setDoc(null); setError(""); setBase(null);
      (async () => {
        const [pdfjs, res] = await Promise.all([loadPdfjs(), bridge.research("pdf", project, { paperId })]);
        if (dead) return;
        if (!res.ok) throw new Error(res.error);
        loaded = await pdfjs.getDocument({
          data: new Uint8Array(res.value),
          cMapUrl: "vendor/pdfjs/cmaps/", cMapPacked: true,
          standardFontDataUrl: "vendor/pdfjs/standard_fonts/",
          isEvalSupported: false,
        }).promise;
        const first = await loaded.getPage(1);
        if (dead) return;
        const vp = first.getViewport({ scale: 1 });
        setBase({ w: vp.width, h: vp.height });
        setDoc(loaded);
      })().catch(e => { if (!dead) setError(String(e?.message ?? e)); });
      return () => { dead = true; try { loaded?.destroy(); } catch { /* gone */ } };
    }, [bridge, project, paperId]);

    React.useEffect(() => {
      const el = scroller.current;
      if (!el || !base) return undefined;
      const measure = () => setFit(Math.max(0.3, Math.min(3, (el.clientWidth - 48) / base.w)));
      measure();
      const ro = new ResizeObserver(measure);
      ro.observe(el);
      return () => ro.disconnect();
    }, [base]);

    const goTo = React.useCallback(n => {
      const el = scroller.current?.querySelector(`[data-page="${n}"]`);
      if (el) scroller.current.scrollTop = el.offsetTop - 12;
    }, []);

    // A page to show (a hit, an anchor, a section): scroll there once the pages are laid out.
    React.useEffect(() => {
      if (!doc || !page) return;
      const id = requestAnimationFrame(() => goTo(page));
      return () => cancelAnimationFrame(id);
    }, [doc, page, nonce, goTo, scale]);

    const mark = React.useMemo(() => (quote ? { quote, nonce } : null), [quote, nonce]);
    const onMarked = React.useCallback((num, found, layer) => {
      if (num !== page) return;
      if (found) {
        setNote("");
        layer.querySelector(".rd-pdf-mark")?.scrollIntoView({ block: "center", behavior: "smooth" });
      } else setNote(T("The passage is on this page, but it could not be marked in the PDF text."));
    }, [page, T]);

    const onScroll = () => {
      const el = scroller.current;
      if (!el) return;
      const mid = el.scrollTop + el.clientHeight * 0.35;
      let best = 1;
      for (const p of el.querySelectorAll(".rd-pdf-page")) { if (p.offsetTop <= mid) best = Number(p.dataset.page); else break; }
      setNow(best);
      onPage?.(best);
    };

    if (error) return <div className="rd-none">{T("This PDF could not be opened")}: {error}</div>;
    if (!doc || !base) return <div className="rd-none">{T("Opening the PDF…")}</div>;
    return (
      <div className="rd-pdf">
        <div className="rd-pdf-bar">
          <button type="button" className="btn icon ghost" onClick={() => goTo(Math.max(1, now - 1))} aria-label={T("Previous page")}>‹</button>
          <input className="rd-pdf-page-input" value={now} aria-label={T("Page")}
            onChange={e => { const n = Number(e.target.value); if (n >= 1 && n <= doc.numPages) { setNow(n); goTo(n); } }} />
          <span>/ {doc.numPages}</span>
          <button type="button" className="btn icon ghost" onClick={() => goTo(Math.min(doc.numPages, now + 1))} aria-label={T("Next page")}>›</button>
          <span className="rd-pdf-gap" />
          <button type="button" className="btn icon ghost" onClick={() => setZoom(Math.max(0.3, +(scale - 0.15).toFixed(2)))} aria-label={T("Zoom out")}>−</button>
          <span className="rd-pdf-zoom">{Math.round(scale * 100)}%</span>
          <button type="button" className="btn icon ghost" onClick={() => setZoom(Math.min(4, +(scale + 0.15).toFixed(2)))} aria-label={T("Zoom in")}>+</button>
          <button type="button" className="btn ghost" onClick={() => setZoom(null)}>{T("Fit width")}</button>
        </div>
        {note && <div className="lb-notice" onClick={() => setNote("")}>{note}</div>}
        <div className="rd-pdf-scroll" ref={setScroller} onScroll={onScroll}>
          {Array.from({ length: doc.numPages }, (_, i) => (
            <PdfPage key={i + 1} doc={doc} num={i + 1} scale={scale} base={base} root={root}
              mark={i + 1 === page ? mark : null} onMarked={onMarked} />
          ))}
        </div>
      </div>
    );
  }
  window.PdfPane = PdfPane;
})();
