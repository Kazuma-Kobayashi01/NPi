/* librarian/lib-hooks.jsx — two parts of the Librarian window with state of their own: copying to the
   clipboard, and the queue of PDFs waiting for a draft. */

/** Copies `text` (and `html`, for a paste into Word or a mail) to the system clipboard. Rust writes it
 *  (clipboard.rs): WebKit refuses `navigator.clipboard` once the click's gesture is spent on an await. */
async function libCopyRich(text, html) {
  try {
    await window.__TAURI__.core.invoke("clipboard_write", { text, html: html || null });
    return true;
  } catch { /* not in the desktop shell: the browser API */ }
  try {
    if (html && window.ClipboardItem) {
      await navigator.clipboard.write([new ClipboardItem({
        "text/plain": new Blob([text], { type: "text/plain" }),
        "text/html": new Blob([html], { type: "text/html" }),
      })]);
    } else await navigator.clipboard.writeText(text);
    return true;
  } catch { return false; }
}

function useLibCopy({ call, selection, shown, entries, styleId, say }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  /** `kind`: "citation" (in text) or "reference" (the list entry) for `ids`, in the chosen style. */
  const cite = async (ids, kind, style = styleId) => {
    const r = await call("format", { style, ids });
    if (!r.ok) { say(r.error); return false; }
    const lines = kind === "citation" ? r.value.citations : r.value.bibliography;
    if (lines.length === 0) { say(T("Nothing to copy")); return false; }
    const text = lines.map(l => l.text).join("\n");
    const html = lines.map(l => `<p>${l.html}</p>`).join("");
    if (await libCopyRich(text, html)) return true;
    say(T("could not copy"));
    return false;
  };
  const copyPlain = async (text, done) => {
    if (!(await libCopyRich(text, null))) { say(T("could not copy")); return false; }
    say(done);
    return true;
  };
  const copyKey = entry => copyPlain(L.citeMarker([entry.citekey]), T("Copied {text}", { text: L.citeMarker([entry.citekey]) }));
  /** "Copy as…": the reference list / citation in `style`, or `[@key]` / BibTeX / RIS / CSL-JSON text, of the selected references or the list. */
  const copyAs = async ({ kind, scope, style, label }) => {
    const ids = scope === "selected" && selection.ids.length > 0 ? selection.ids : shown.map(e => e.id);
    if (ids.length === 0) return false;
    if (kind === "citekey") {
      const keys = ids.map(id => entries.find(e => e.id === id)?.citekey).filter(Boolean);
      return copyPlain(L.citeMarker(keys), T("Copied {text}", { text: L.citeMarker(keys) }));
    }
    if (kind === "bibtex" || kind === "ris" || kind === "csl-json") {
      const r = await call("export_text", { format: kind, ids });
      if (!r.ok) { say(r.error); return false; }
      return copyPlain(r.value, T("Copied {n} references as {format}", { n: ids.length, format: kind === "bibtex" ? "BibTeX" : kind === "ris" ? "RIS" : "CSL-JSON" }));
    }
    if (!(await cite(ids, kind, style))) return false;
    say(T("Copied {n} in {style}", { n: ids.length, style: label }));
    return true;
  };
  return { cite, copyAs, copyKey };
}

/** PDFs dropped on the window or picked in its dialog: one draft at a time, the next when it is added or discarded. */
function useLibDrop({ call, draft, setDraft, setSelectedId, say }) {
  const T = window.OMP_I18N.t;
  const [dropping, setDropping] = React.useState(false);
  const [queue, setQueue] = React.useState([]);
  const start = React.useCallback(async path => {
    const res = await call("pdf_dropped", { path });
    if (!res.ok) return say(res.error);
    setSelectedId(null);
    return setDraft(res.value);
  }, [call]);   // eslint-disable-line react-hooks/exhaustive-deps
  React.useEffect(() => {
    if (!window.OMP_PANEL) return undefined;
    let off = () => {};
    window.__TAURI__?.webview?.getCurrentWebview?.().onDragDropEvent(ev => {
      const { type, paths } = ev.payload;
      setDropping(type === "enter" || type === "over");
      if (type !== "drop") return;
      const pdfs = (paths ?? []).filter(p => /\.pdf$/i.test(p));
      if (pdfs.length === 0) say(T("Only PDF files can be added here."));
      else setQueue(q => [...q, ...pdfs]);
    }).then(un => { off = un; }).catch(() => {});
    return () => off();
  }, []);   // eslint-disable-line react-hooks/exhaustive-deps
  React.useEffect(() => {
    if (draft || queue.length === 0) return;
    const [next, ...rest] = queue;
    setQueue(rest);
    start(next);
  }, [draft, queue, start]);
  return { dropping, waiting: queue.length, enqueue: paths => setQueue(q => [...q, ...paths]), skipWaiting: () => setQueue([]) };
}

/** Dragging reference rows onto a group in the sidebar. Pointer events, not HTML5 drag and drop: the panel
 *  window's native file drop takes every HTML5 drag before the page sees it. A move under 5 px is a click. */
function useLibRowDrag({ onDrop }) {
  const [drag, setDrag] = React.useState(null);   // { ids, x, y, over } once the pointer has really moved
  const moved = React.useRef(false);
  const begin = (ev, ids) => {
    if (ev.button !== 0) return;
    const from = { x: ev.clientX, y: ev.clientY };
    let live = null;
    const groupAt = (x, y) => Number(document.elementFromPoint(x, y)?.closest?.("[data-group]")?.dataset.group) || null;
    const move = e => {
      if (!live && Math.hypot(e.clientX - from.x, e.clientY - from.y) < 5) return;
      live = { ids, x: e.clientX, y: e.clientY, over: groupAt(e.clientX, e.clientY) };
      moved.current = true;
      setDrag(live);
    };
    const end = e => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      setDrag(null);
      if (live && e.type === "pointerup") {
        const group = groupAt(e.clientX, e.clientY);
        if (group) onDrop(ids, group);
      }
      setTimeout(() => { moved.current = false; }, 0);   // the click that follows a drag is not a selection
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  };
  return { drag, begin, wasDrag: () => moved.current };
}

Object.assign(window, { useLibCopy, useLibDrop, useLibRowDrag });
