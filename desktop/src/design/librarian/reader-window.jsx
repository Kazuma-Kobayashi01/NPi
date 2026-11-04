/* librarian/reader-window.jsx — the reader of one Academic project's Research Core index
   (docs/architecture/research-core.md): the papers on the left with their sections, one page of
   a paper in the middle, and the full-text search above. Opened as its own window
   (panel kind "reader"); `?anchor=` or the `reader://goto` event takes it to a citation's evidence.
   Everything is read through `bridge.research` (librarian/reader.rs); it changes nothing but the
   index, which "Index project" refreshes. */

/** A heading as the PDF's Markdown left it (`Λ<sub>σ</sub>`, `<i>…</i>`): the tags dropped, subscripts and superscripts kept as text. */
const plainHeading = text => String(text ?? "").replace(/<\/?(?:sub|sup|i|b|em|strong|span)[^>]*>/gi, "").replace(/\s+/g, " ").trim();

function ReaderWindow({ bridge, project, anchor, path, onClose }) {
  const T = window.OMP_I18N.t;
  const { Icon } = window;
  const [status, setStatus] = React.useState(null);
  const [papers, setPapers] = React.useState([]);
  const [paperId, setPaperId] = React.useState(null);
  const [sections, setSections] = React.useState([]);
  const [page, setPage] = React.useState(null);
  const [view, setView] = React.useState(null);
  const [mark, setMark] = React.useState(null);   // the block to highlight
  const [query, setQuery] = React.useState("");
  const [hits, setHits] = React.useState(null);   // null = not searching
  const [notice, setNotice] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [tab, setTab] = React.useState("pdf");      // pdf | text | summary
  const [jump, setJump] = React.useState({ page: 1, quote: null, nonce: 0 });
  const [summaries, setSummaries] = React.useState([]);
  const [comparisons, setComparisons] = React.useState([]);
  const [showTables, setShowTables] = React.useState(false);
  const rootRef = React.useRef(null);
  window.useModalFocus(rootRef);
  const wc = window.useWindowChrome(onClose, rootRef);
  const call = React.useCallback((name, args) => bridge.research(name, project, args), [bridge, project]);
  const paper = papers.find(p => p.id === paperId) ?? null;

  const loadPapers = React.useCallback(async () => {
    const [st, list] = await Promise.all([call("index_status"), call("papers")]);
    if (st.ok) setStatus(st.value); else setNotice(st.error);
    if (list.ok) setPapers(list.value); else setNotice(list.error);
    const cmp = await call("comparisons");
    if (cmp.ok) setComparisons(cmp.value);
    return list.ok ? list.value : [];
  }, [call]);

  const openPaper = React.useCallback(async (id, wantPage, blockId, list) => {
    const row = (list ?? papers).find(p => p.id === id);
    setPaperId(id); setMark(blockId ?? null); setHits(null); setShowTables(false);
    setTab(t => (t === "summary" && blockId == null ? t : row?.kind === "pdf" ? "pdf" : "text"));
    const [secs, sums] = await Promise.all([call("sections", { paperId: id }), call("summaries", { paperId: id })]);
    setSections(secs.ok ? secs.value : []);
    setSummaries(sums.ok ? sums.value : []);
    const target = row && row.kind === "pdf" ? Math.min(Math.max(wantPage ?? 1, 1), Math.max(row.pageCount, 1)) : null;
    const res = await call("page", { paperId: id, page: target });
    if (!res.ok) return setNotice(res.error);
    setPage(target); setView(res.value);
    // The passage of the block to show, for the PDF view to mark on its page.
    const quote = blockId != null ? res.value.blocks.find(b => b.id === blockId)?.text ?? null : null;
    setJump({ page: target ?? 1, quote, nonce: Date.now() });
  }, [call, papers]);

  const goAnchor = React.useCallback(async (anchorId, list) => {
    const res = await call("anchor", { anchorId });
    if (!res.ok) return setNotice(res.error);
    if (!res.value) return setNotice(T("That evidence is no longer in the index"));
    if (res.value.stale) setNotice(T("The file changed since it was indexed: this evidence may be out of date"));
    await openPaper(res.value.paperId, res.value.page ?? 1, res.value.blockId, list);
  }, [call, openPaper, T]);

  /** Opens the file at `rel`; one the index has not reached yet is indexed first. */
  const openPath = React.useCallback(async (rel, list) => {
    const known = list.find(p => p.path === rel);
    if (known) return openPaper(known.id, 1, null, list);
    setBusy(true);
    const res = await call("index", { text: false });
    setBusy(false);
    if (!res.ok) return setNotice(res.error);
    const fresh = await loadPapers();
    const row = fresh.find(p => p.path === rel);
    if (row) return openPaper(row.id, 1, null, fresh);
    return setNotice(T("{path} could not be read", { path: rel }));
  }, [call, loadPapers, openPaper, T]);

  React.useEffect(() => {
    let live = true;
    (async () => {
      const list = await loadPapers();
      if (!live) return;
      if (anchor) await goAnchor(anchor, list);
      else if (path) await openPath(path, list);
      else if (list.length) await openPaper(list[0].id, 1, null, list);
    })();
    return () => { live = false; };
  }, []);   // eslint-disable-line react-hooks/exhaustive-deps

  React.useEffect(() => {
    let off = () => {};
    window.__TAURI__?.event?.listen("reader://goto", ev => { if (ev.payload) goAnchor(ev.payload); })
      .then(un => { off = un; }).catch(() => {});
    return () => off();
  }, [goAnchor]);

  // A file the Librarian asks to read in this already open window.
  React.useEffect(() => {
    let off = () => {};
    window.__TAURI__?.event?.listen("reader://open", ev => {
      if (ev.payload) openPath(ev.payload, papers);
    }).then(un => { off = un; }).catch(() => {});
    return () => off();
  }, [papers, openPath]);

  // Esc leaves the search. The window itself closes the way a window does (Cmd+W, the red button);
  // only the overlay form inside the main window closes on Esc.
  React.useEffect(() => {
    const onKey = e => {
      if (e.key !== "Escape") return;
      if (hits) { e.preventDefault(); setHits(null); setQuery(""); } else if (!window.OMP_PANEL) { e.preventDefault(); onClose(); }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose, hits]);

  React.useEffect(() => {
    if (mark == null) return;
    rootRef.current?.querySelector(`[data-block="${mark}"]`)?.scrollIntoView({ block: "center" });
  }, [view, mark]);

  const indexNow = async () => {
    setBusy(true); setNotice("");
    const res = await call("index", {});
    setBusy(false);
    if (!res.ok) return setNotice(res.error);
    const r = res.value;
    setNotice(T("Indexed: {a} added, {u} updated, {o} read by OCR, {s} without text", { a: r.added, u: r.updated, o: r.ocr.length, s: r.scanned.length }));
    const list = await loadPapers();
    if (list.length && paperId == null) await openPaper(list[0].id, 1, null, list);
  };

  const search = async e => {
    e.preventDefault();
    const q = query.trim();
    if (!q) return setHits(null);
    const res = await call("search", { query: q, limit: 40 });
    if (!res.ok) return setNotice(res.error);
    setHits(res.value);
  };

  const goPage = delta => paper && openPaper(paper.id, (page ?? 1) + delta, null);
  const unindexed = status && !status.indexed;

  return (
    <div className="st-overlay">
      <div className="st-modal rd-modal" role="dialog" aria-label={T("Reader")} ref={rootRef} tabIndex={-1}>
        <div className="st-modal-head" {...wc.headProps}>
          {wc.lights}
          <span className="st-modal-title">{T("Reader")} · {project}</span>
          <form className="rd-search" onSubmit={search}>
            <input className="lb-input" value={query} onChange={e => { setQuery(e.target.value); if (!e.target.value) setHits(null); }}
              placeholder={T("Search the papers")} aria-label={T("Search the papers")} />
          </form>
          <button type="button" className="btn ghost" onClick={indexNow} disabled={busy}>{busy ? T("Indexing…") : T("Index project")}</button>
        </div>
        {notice && <div className="lb-notice" onClick={() => setNotice("")}>{notice}</div>}
        {unindexed && papers.length === 0 ? (
          <div className="rd-empty">
            <p>{T("This project is not indexed yet. Indexing reads the PDFs in papers/ (and your notes) so they can be searched and cited by page.")}</p>
            <button type="button" className="btn primary" onClick={indexNow} disabled={busy}>{busy ? T("Indexing…") : T("Index project")}</button>
          </div>
        ) : (
          <div className="rd-body">
            <nav className="rd-side">
              {hits ? (
                <>
                  <div className="lb-side-head"><span>{T("{n} results", { n: hits.length })}</span>
                    <button type="button" className="btn icon ghost" onClick={() => setHits(null)} aria-label={T("Close")}><Icon name="x" size={11} /></button></div>
                  {hits.length === 0 && <div className="rd-none">{T("No matches")}</div>}
                  {hits.map(h => (
                    <button key={h.blockId} type="button" className="rd-hit-row" onClick={() => openPaper(h.paperId, h.page ?? 1, h.blockId)}>
                      <span className="rd-hit-where">{h.path}{h.page ? ` · p.${h.page}` : ""}</span>
                      <span className="rd-hit-snippet">{h.snippet}</span>
                    </button>
                  ))}
                </>
              ) : (
                <>
                  <div className="lb-side-head"><span>{T("Comparisons")}</span></div>
                  <button type="button" className={`rd-paper${showTables ? " on" : ""}`} onClick={() => setShowTables(true)}>
                    <span className="rd-paper-title">{T("Comparison tables")}</span>
                    <span className="rd-paper-meta">{comparisons.length}</span>
                  </button>
                  <div className="lb-side-head"><span>{T("Papers")}</span></div>
                  {papers.map(p => (
                    <div key={p.id}>
                      <button type="button" className={`rd-paper${p.id === paperId ? " on" : ""}`} onClick={() => openPaper(p.id, 1, null)}>
                        <span className="rd-paper-title">{plainHeading(p.title) || p.path}</span>
                        <span className="rd-paper-meta">
                          {p.parseStatus !== "ok" && <span className={`rd-badge ${p.parseStatus}`} title={p.parseNote}>{p.parseStatus === "ocr" ? "OCR" : p.parseStatus}</span>}
                          {p.staleAnchors > 0 && <span className="rd-badge stale">{T("stale")}</span>}
                          {p.kind === "pdf" ? `${p.pageCount} p.` : T("text")}
                        </span>
                      </button>
                      {p.id === paperId && sections.map(s => (
                        <button key={s.id} type="button" className="rd-section" style={{ paddingLeft: 22 + Math.min(s.level, 4) * 10 }}
                          onClick={() => s.page && openPaper(p.id, s.page, null)} title={plainHeading(s.heading)}>{plainHeading(s.heading)}</button>
                      ))}
                    </div>
                  ))}
                </>
              )}
            </nav>
            <section className="rd-main">
              {showTables ? (
                <ReaderTables comparisons={comparisons} onGo={goAnchor} onDelete={async id => { const r = await call("comparison_delete", { id }); if (r.ok) setComparisons(c => c.filter(x => x.id !== id)); else setNotice(r.error); }} />
              ) : paper && view ? (
                <>
                  <div className="rd-bar">
                    <strong className="rd-bar-title">{plainHeading(paper.title) || paper.path}</strong>
                    {paper.parseStatus === "scanned" && <span className="rd-badge scanned">{T("No text layer")}</span>}
                    <span className="mode-seg rd-tabs" role="tablist">
                      {paper.kind === "pdf" && <button type="button" role="tab" aria-selected={tab === "pdf"} className={tab === "pdf" ? "on" : ""} onClick={() => setTab("pdf")}>PDF</button>}
                      <button type="button" role="tab" aria-selected={tab === "text"} className={tab === "text" ? "on" : ""} onClick={() => setTab("text")}>{T("Text")}</button>
                      <button type="button" role="tab" aria-selected={tab === "summary"} className={tab === "summary" ? "on" : ""} onClick={() => setTab("summary")}>{T("Summary")}{summaries.length ? ` (${summaries.length})` : ""}</button>
                    </span>
                    {tab === "text" && paper.kind === "pdf" && (
                      <span className="rd-pager">
                        <button type="button" className="btn icon ghost" onClick={() => goPage(-1)} disabled={(page ?? 1) <= 1} aria-label={T("Previous page")}>‹</button>
                        <span>{page} / {Math.max(paper.pageCount, 1)}</span>
                        <button type="button" className="btn icon ghost" onClick={() => goPage(1)} disabled={(page ?? 1) >= paper.pageCount} aria-label={T("Next page")}>›</button>
                      </span>
                    )}
                  </div>
                  {paper.parseStatus === "ocr" && <div className="lb-notice" title={paper.parseNote}>{T("The text of this paper was recognised by OCR and may contain mistakes: check numbers and quotes against the page.")}</div>}
                  {tab === "summary" ? (
                    <ReaderSummaries summaries={summaries} onGo={goAnchor} onPage={(p, id) => openPaper(paper.id, p, id)} />
                  ) : tab === "pdf" && paper.kind === "pdf" ? (
                    <PdfPane key={paper.id} bridge={bridge} project={project} paperId={paper.id} page={jump.page} quote={jump.quote} nonce={jump.nonce} onPage={setPage} />
                  ) : (
                    <div className="rd-page">
                      {view.blocks.length === 0 && <div className="rd-none">{paper.parseNote || T("No text on this page")}</div>}
                      {view.blocks.map(b => b.kind === "heading"
                        ? <h4 key={b.id} data-block={b.id} className={`rd-h${mark === b.id ? " rd-mark" : ""}`}>{plainHeading(b.text)}</h4>
                        : <p key={b.id} data-block={b.id} className={`rd-p${mark === b.id ? " rd-mark" : ""}${b.anchorId ? " has-anchor" : ""}`}>{b.text}</p>)}
                      {view.truncated && <div className="rd-none">{T("Only the first blocks are shown")}</div>}
                    </div>
                  )}
                </>
              ) : <div className="rd-none">{papers.length ? T("Pick a paper") : T("No papers indexed yet")}</div>}
            </section>
          </div>
        )}
      </div>
    </div>
  );
}
window.ReaderWindow = ReaderWindow;

/** Status words of a checked claim: only a quote found on the page is "matched". */
function ReaderStatus({ status }) {
  const T = window.OMP_I18N.t;
  const label = { matched: T("Matches the original"), unverified: T("Not found in the original"), model: T("Model's reading"), not_reported: T("Not reported"), not_comparable: T("Not comparable") }[status] ?? status;
  return <span className={`rd-status ${status}`}>{label}</span>;
}

/** The summaries of a paper: L2 first, then the sections'; each fact says what backs it. */
function ReaderSummaries({ summaries, onGo, onPage }) {
  const T = window.OMP_I18N.t;
  if (summaries.length === 0) return <div className="rd-none">{T("No summary yet. Ask the Copilot to summarize this paper; it saves the result here.")}</div>;
  return (
    <div className="rd-page">
      <p className="rd-hint">{T("A summary never replaces the paper: numbers, formulas and quotes go back to the original page.")}</p>
      {summaries.map(sm => (
        <article key={sm.id} className="rd-summary">
          <h4 className="rd-h">{sm.level === "paper" ? T("Paper summary") : plainHeading(sm.heading)}
            {sm.stale && <span className="rd-badge stale">{T("stale: the file changed")}</span>}</h4>
          <p className="rd-p">{sm.body}</p>
          {sm.facts.length > 0 && (
            <ul className="rd-facts">
              {sm.facts.map((f, i) => (
                <li key={i}>
                  <span>{f.text}</span>
                  <ReaderStatus status={f.status} />
                  {f.page && <button type="button" className="rd-link" onClick={() => (f.anchorId ? onGo(f.anchorId) : onPage(f.page, null))}>p.{f.page}</button>}
                </li>
              ))}
            </ul>
          )}
          {sm.coverage && <p className="rd-hint">{T("Not covered")}: {sm.coverage}</p>}
        </article>
      ))}
    </div>
  );
}

/** The comparison tables: a cell shows its value and whether the paper backs it. */
function ReaderTables({ comparisons, onGo, onDelete }) {
  const T = window.OMP_I18N.t;
  if (comparisons.length === 0) return <div className="rd-none">{T("No comparison tables yet. Ask the Copilot to compare papers; it saves the table here.")}</div>;
  return (
    <div className="rd-page rd-tables">
      {comparisons.map(c => (
        <article key={c.id} className="rd-summary">
          <h4 className="rd-h">{c.title}
            <button type="button" className="btn ghost rd-del" onClick={() => onDelete(c.id)}>{T("Delete")}</button></h4>
          {c.question && <p className="rd-hint">{c.question}</p>}
          <div className="rd-table-wrap">
            <table className="rd-table">
              <thead><tr><th />{c.columns.map(col => <th key={col.key}>{col.label}</th>)}</tr></thead>
              <tbody>
                {c.rows.map((row, i) => (
                  <tr key={i}>
                    <th>{row.label}</th>
                    {c.columns.map(col => {
                      const cell = row.cells[col.key];
                      if (!cell) return <td key={col.key} />;
                      return (
                        <td key={col.key} className={`rd-cell ${cell.status}`}>
                          <span>{cell.value || "—"}</span>
                          <ReaderStatus status={cell.status} />
                          {cell.anchorId && <button type="button" className="rd-link" onClick={() => onGo(cell.anchorId)}>{cell.page ? `p.${cell.page}` : T("source")}</button>}
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </article>
      ))}
    </div>
  );
}
window.ReaderStatus = ReaderStatus;
