/* librarian/librarian-window.jsx — the Librarian of one Academic project:
   a floating window with groups and tags on the left, the searchable list in
   the middle and one reference's fields on the right (docs/PLAN_LIBRARIAN.md,
   L1). The data is Rust's (library_* commands through `bridge.library`);
   what is shown and how it is filtered is app/librarian.js. */

function LibrarianWindow({ bridge, project, profile, onClose }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const { Icon } = window;
  const [entries, setEntries] = React.useState([]);
  const [groups, setGroups] = React.useState([]);
  const [loaded, setLoaded] = React.useState(false);
  const [scope, setScope] = React.useState({ group: null, tag: null });
  const [query, setQuery] = React.useState("");
  const [sort, setSortState] = React.useState(() => {
    try { const v = JSON.parse(localStorage.getItem("npi.librarian.sort")); if (L.SORT_KEYS.includes(v?.key)) return v; } catch { /* the default */ }
    return { key: "added", dir: "desc" };
  });
  const setSort = next => { setSortState(next); try { localStorage.setItem("npi.librarian.sort", JSON.stringify(next)); } catch { /* not kept */ } };
  const [selection, setSelection] = React.useState({ ids: [], anchor: null });
  const selectedId = selection.ids.length === 1 ? selection.ids[0] : null;
  const setSelectedId = id => setSelection({ ids: id === null ? [] : [id], anchor: id });
  const detail = React.useRef(null);    // { flush }: the open form, saved before the selection moves away
  const [align, setAlign] = React.useState(null);   // the references being aligned with Crossref
  const [importing, setImporting] = React.useState(false);
  const [notice, setNotice] = React.useState("");
  const [trash, setTrash] = React.useState(null);   // null = closed, else the removed references
  const [report, setReport] = React.useState(null);   // the citation check of a draft
  const [draft, setDraft] = React.useState(null);   // a PDF draft awaiting the user's confirmation
  const [styles, setStyles] = React.useState([]);
  const styleKey = `npi.librarian.style.${project}`;
  const [styleId, setStyleId] = React.useState(() => { try { return localStorage.getItem(styleKey) || "apa"; } catch { return "apa"; } });
  const rootRef = React.useRef(null);
  window.useModalFocus(rootRef);
  const wc = window.useWindowChrome(onClose, rootRef);

  const call = React.useCallback((name, args) => bridge.library(name, project, args), [bridge, project]);
  const say = text => setNotice(text);
  // A status line goes by itself; a click dismisses it sooner.
  React.useEffect(() => {
    if (!notice) return undefined;
    const timer = setTimeout(() => setNotice(""), 7000);
    return () => clearTimeout(timer);
  }, [notice]);

  const greeted = React.useRef(false);   // the count of waiting suggestions is said once, at opening
  const reload = React.useCallback(async () => {
    const [list, grp] = await Promise.all([call("list"), call("groups")]);
    if (list.ok) {
      setEntries(list.value);
      const waiting = list.value.reduce((n, e) => n + (e.suggestions?.length ?? 0), 0);
      if (waiting > 0 && !greeted.current) say(T("{n} suggestions from the Copilot are waiting", { n: waiting }));
    } else say(list.error);
    if (grp.ok) setGroups(grp.value);
    greeted.current = true;
    setLoaded(true);
  }, [call]);
  React.useEffect(() => { reload(); }, [reload]);
  // An agent may have changed the library meanwhile. An open form with edits waits, or it would be replaced.
  React.useEffect(() => {
    const again = () => { if (!detail.current?.dirty()) reload(); };
    window.addEventListener("focus", again);
    return () => window.removeEventListener("focus", again);
  }, [reload]);
  React.useEffect(() => { call("styles").then(r => { if (r.ok) setStyles(r.value); }); }, [call]);
  const pickStyle = id => { setStyleId(id); try { localStorage.setItem(styleKey, id); } catch {} };
  const addStyle = async () => {
    const r = await call("style_add_pick");
    if (!r.ok) return say(r.error);
    if (!r.value) return;
    setStyles((await call("styles")).value ?? styles);
    pickStyle(r.value.id);
    say(T("Added the style {name}", { name: r.value.name }));
  };
  // Esc steps back: out of the search, then the selection. Its own window closes the way a window does
  // (Cmd+W, the red button), never from a key meant for a field; only the overlay form closes on it.
  React.useEffect(() => {
    const onKey = e => {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        document.querySelector(".lb-search input")?.select();
        return;
      }
      if (e.key !== "Escape") return;
      e.preventDefault();
      if (query) setQuery("");
      else if (selection.ids.length > 1) setSelectedId(selection.ids[0]);
      else if (!window.OMP_PANEL) onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose, query, selection]);

  // Cmd+W and the red button: the open form is saved first.
  React.useEffect(() => {
    if (!window.OMP_PANEL) return undefined;
    let off = () => {};
    window.__TAURI__?.window?.getCurrentWindow?.().onCloseRequested(async ev => {
      if (detail.current && !(await detail.current.flush())) ev.preventDefault();
    }).then(un => { off = un; }).catch(() => {});
    return () => off();
  }, []);

  const refreshGroups = async () => { const g = await call("groups"); if (g.ok) setGroups(g.value); };
  /** The open form is saved before anything that changes the reference under it, or re-opens the form
   *  from what is stored and loses the edits. False when it could not be saved: its error shows and nothing moves on. */
  const settled = async () => !detail.current || detail.current.flush();
  // A new PDF reaches the Reader and the full-text search by itself, a moment after it is filed.
  const indexTimer = React.useRef(null);
  const indexSoon = () => {
    clearTimeout(indexTimer.current);
    indexTimer.current = setTimeout(() => { bridge.research("index", project, { text: false }); }, 1500);
  };
  const { dropping, waiting, enqueue, skipWaiting } = useLibDrop({ call, draft, setDraft, setSelectedId, say });
  const replace = updated => setEntries(list => list.map(e => (e.id === updated.id ? updated : e)));
  // Full-text matches (the project's FTS5 index: short Chinese words, ranked) come first; the substring
  // filter still finds what the index does not (a fragment inside a word), and stands alone when the
  // project has no index yet.
  const [ranked, setRanked] = React.useState(null);
  React.useEffect(() => {
    const q = query.trim();
    if (!q) { setRanked(null); return undefined; }
    let live = true;
    const timer = setTimeout(async () => {
      const res = await call("search", { query: q });
      if (live) setRanked(res.ok && Array.isArray(res.value) ? res.value : null);
    }, 160);
    return () => { live = false; clearTimeout(timer); };
  }, [query, call, entries]);
  const shown = React.useMemo(() => {
    const plain = L.visible(entries, { ...scope, query });
    // The list's own sort wins once the user picks one; left at "added", a search shows best matches first.
    if (!ranked || !query.trim() || sort.key !== "added") return L.sorted(plain, sort.key, sort.dir);
    const inScope = new Map(L.visible(entries, { ...scope, query: "" }).map(e => [e.id, e]));
    const byRank = ranked.map(id => inScope.get(id)).filter(Boolean);
    const seen = new Set(byRank.map(e => e.id));
    // Ranked hits keep the index's order; sorting columns still apply to the substring-only rest.
    return [...byRank, ...L.sorted(plain.filter(e => !seen.has(e.id)), sort.key, sort.dir)];
  }, [entries, scope, query, sort, ranked]);
  const tags = React.useMemo(() => L.tagCounts(entries), [entries]);
  const smart = React.useMemo(() => L.smartCounts(entries), [entries]);
  const selected = entries.find(e => e.id === selectedId) ?? null;
  const selectedMany = React.useMemo(() => entries.filter(e => selection.ids.includes(e.id)), [entries, selection]);
  /** References dragged onto a group: each joins it (a reference dragged out of a selection takes the selection along). */
  const dropOnGroup = async (ids, group) => {
    if (!(await settled())) return;
    const name = groups.find(g => g.id === group)?.name ?? "";
    for (const id of ids) {
      const r = await call("member", { id, group, member: true });
      if (!r.ok) return say(r.error);
      replace(r.value);
    }
    await refreshGroups();
    say(T("Added {n} to {group}", { n: ids.length, group: name }));
  };
  const rowDrag = useLibRowDrag({ onDrop: dropOnGroup });
  const dragIds = id => (selection.ids.includes(id) ? selection.ids : [id]);
  const { copyAs, cite, copyKey } = useLibCopy({ call, selection, shown, entries, styleId, say });
  const batch = useLibBatch({ call, chosen: selectedMany, replace, setEntries, setSelection, refreshGroups, say });

  // A filter that hides part of a multiple selection takes those out: nothing is changed in bulk unseen.
  React.useEffect(() => {
    if (selection.ids.length < 2) return;
    const inList = new Set(shown.map(e => e.id));
    const kept = selection.ids.filter(id => inList.has(id));
    if (kept.length !== selection.ids.length) setSelection({ ids: kept, anchor: kept[0] ?? null });
  }, [shown]);   // eslint-disable-line react-hooks/exhaustive-deps
  React.useEffect(() => {
    if (selectedId !== null) document.querySelector(`[data-ref="${selectedId}"]`)?.scrollIntoView({ block: "nearest" });
  }, [selectedId]);

  // The selection never moves away from an unsaved form: it is saved first, and stays if that fails.
  const clickRow = async (id, mods) => {
    if (draft) return say(T("Add the draft to the library or discard it first."));
    if (!(await settled())) return;
    setSelection(sel => L.clickSelect(sel, id, shown.map(e => e.id), mods));
  };
  const stepRow = async (delta, extend) => {
    if (draft || !(await settled())) return;
    const order = shown.map(e => e.id);
    const from = selection.ids[selection.ids.length - 1] ?? null;
    const id = L.stepSelect(order, from, delta);
    if (id === null) return;
    setSelection(sel => L.clickSelect(sel, id, order, { range: extend }));
    document.querySelector(`[data-ref="${id}"]`)?.scrollIntoView({ block: "nearest" });
  };
  const selectAll = async () => {
    if (draft || !(await settled())) return;
    setSelection({ ids: shown.map(e => e.id), anchor: shown[0]?.id ?? null });
  };

  // A new reference is a form until it is saved, so an abandoned one leaves nothing behind.
  const addReference = async () => {
    if (!(await settled())) return;
    setSelectedId(null);
    setDraft({ blank: true, csl: { type: "article-journal" }, guessed: [] });
  };
  const importDone = async report => {
    say(T("Added {n}; {m} already in the library", { n: report.added.length, m: report.duplicates.length }));
    await reload();
  };
  const importFile = async () => {
    const res = await call("import_pick");
    if (!res.ok) return say(res.error);
    if (res.value === null) return undefined;
    setImporting(false);
    return importDone(res.value);
  };
  const exportAs = async format => {
    const ids = selection.ids.length > 1 ? selection.ids : shown.map(e => e.id);
    const res = await call("export_save", { format, ids });
    if (!res.ok) say(res.error); else if (res.value) say(T("Saved {n} references", { n: ids.length }));
  };
  // Offers this project's library to agents over MCP (`<project>/.mcp.json`; librarian/mcp.rs).
  const connectAgents = async () => {
    const res = await call("mcp_register");
    if (!res.ok) return say(res.error);
    say(T("Registered npi-librarian and npi-copilot in {path}. Agents started in this project folder (npi in a terminal, for one) can now use the library and hand work to the Copilot.", { path: res.value.path }));
  };
  const importPdf = async () => {
    const res = await call("pdf_pick");
    if (!res.ok) return say(res.error);
    return enqueue(res.value);
  };
  const attachPick = async entry => {
    if (!(await settled())) return undefined;
    const res = await call("attach_pick", { id: entry.id });
    if (!res.ok) return say(res.error);
    if (res.value) { replace(res.value); say(T("Attached")); indexSoon(); }
    return undefined;
  };
  const attachDuplicate = async existing => {
    if (draft.blank) { setDraft(null); return setSelectedId(existing.id); }
    const res = await call("attach_pdf", { id: existing.id, hash: draft.hash });
    if (!res.ok) return say(res.error);
    replace(res.value); setDraft(null); setSelectedId(existing.id);
    indexSoon();
    return say(T("Attached to {key}", { key: existing.citekey }));
  };
  const locateAttachment = async a => {
    if (!(await settled())) return undefined;
    const res = await call("attachment_locate", { id: a.id });
    if (!res.ok) return say(res.error);
    if (res.value) { replace(res.value); say(T("Found the file again")); indexSoon(); }
    return undefined;
  };
  const openAttachment = async a => {
    const res = await call("attachment_open", { id: a.id });
    if (!res.ok) say(res.error);
  };
  const removeAttachment = async a => {
    if (!(await settled())) return undefined;
    const res = await call("attachment_remove", { id: a.id });
    if (!res.ok) return say(res.error);
    await reload();
  };
  const lookup = args => call("lookup", args);
  // Page 1 of the reference's PDF to the model (librarian/page_one.rs); the detail pane asked first.
  const readPageOne = entry => call("page_one", { citekey: entry.citekey, profile: profile || null });
  const openTrash = async () => {
    const r = await call("trash");
    if (!r.ok) return say(r.error);
    setTrash(r.value);
  };
  const restore = async item => {
    const r = await call("restore", { id: item.id });
    if (!r.ok) return say(r.error);
    setEntries(list => (list.some(e => e.id === r.value.id) ? list : [...list, r.value]));
    setTrash(t => (t ?? []).filter(x => x.id !== item.id));
    await refreshGroups();
    say(T("Restored {key}", { key: item.citekey }));
  };
  const resolveSuggestion = async (sg, accept) => {
    if (!(await settled())) return undefined;
    const r = await call("suggestion_resolve", { id: sg.id, accept });
    if (!r.ok) return say(r.error);
    replace(r.value);
    if (accept && sg.kind === "group") await refreshGroups();
    return undefined;
  };
  const checkDraft = async () => {
    const r = await call("check_draft_pick");
    if (!r.ok) return say(r.error);
    if (r.value) setReport(r.value);
  };
  const citeDraft = async () => {
    const r = await call("cite_draft", { style: styleId });
    if (!r.ok) return say(r.error);
    if (!r.value) return undefined;
    return say(r.value.missing.length
      ? T("Cited {n}; not in the library: {keys}", { n: r.value.cited, keys: r.value.missing.join(", ") })
      : T("Cited {n} in the draft", { n: r.value.cited }));
  };
  const saveDraft = async csl => {
    if (draft.blank) {
      if (!String(csl.title ?? "").trim()) return { ok: false, error: T("A reference needs a title") };
      const res = await call("import", { format: "csl-json", text: JSON.stringify([csl]) });
      if (!res.ok) return res;
      if (res.value.duplicates.length > 0) {
        const of = res.value.duplicates[0].of;
        setDraft(d => ({ ...d, duplicateOf: entries.find(e => e.id === of) ?? { id: of, citekey: "" } }));
        return { ok: false, error: T("Already in the library") };
      }
      const added = res.value.added[0];
      setEntries(list => [...list, added]);
      setScope({ group: null, tag: null }); setQuery("");
      setDraft(null); setSelectedId(added.id);
      return { ok: true };
    }
    const res = await call("add_pdf", { csl, hash: draft.hash });
    if (!res.ok) return res;
    if (res.value.duplicate) {
      setDraft(d => ({ ...d, duplicateOf: res.value.duplicate }));
      return { ok: false, error: T("Already in the library") };
    }
    setEntries(list => [...list, res.value.entry]);
    setScope({ group: null, tag: null }); setQuery("");
    setDraft(null); setSelectedId(res.value.entry.id);
    indexSoon();
    return { ok: true };
  };
  const save = async (entry, csl, tagList, notes) => {
    if (entry.draft) return saveDraft(csl);
    if (JSON.stringify(csl) !== JSON.stringify(entry.csl)) {
      const r = await call("update", { id: entry.id, csl });
      if (!r.ok) return r;
      replace(r.value);
    }
    const m = await call("meta", { id: entry.id, tags: tagList, notes });
    if (!m.ok) return m;
    replace(m.value);
    return { ok: true };
  };
  const member = async (entry, group, on) => {
    if (!(await settled())) return undefined;
    const r = await call("member", { id: entry.id, group, member: on });
    if (!r.ok) return say(r.error);
    replace(r.value);
    return refreshGroups();
  };
  const remove = async entry => {
    const r = await call("remove", { id: entry.id });
    if (!r.ok) return say(r.error);
    setEntries(list => list.filter(e => e.id !== entry.id));
    setSelectedId(null);
    if (trash) setTrash(null);
    say(T("Moved {key} to the trash", { key: entry.citekey }));
    return refreshGroups();
  };
  const rename = async (entry, key) => {
    if (!(await settled())) return { ok: false };
    const r = await call("citekey", { id: entry.id, key });
    if (!r.ok) { say(r.error); return r; }   // said here: the form may have been re-opened by now
    replace(r.value);
    return { ok: true };
  };
  const readPdf = att => window.OMP_OPEN_PANEL("reader", { project, title: `Reader · ${project}`, path: att.path });
  const merge = async (keep, other) => {
    const r = await call("merge", { keep, other });
    if (!r.ok) return say(r.error);
    setSelectedId(keep);
    await reload();
    say(T("Merged into {key}; the other one is in the trash", { key: r.value.citekey }));
  };
  /** Aligns several selected references; otherwise what the list shows that lacks metadata (one
   *  open reference has "Look up online" in its form). */
  const startAlign = async () => {
    if (!(await settled())) return undefined;
    const pick = selection.ids.length > 1 ? selectedMany : shown.filter(L.incomplete);
    if (pick.length === 0) return say(T("Every reference in the list has its metadata."));
    return setAlign(pick);
  };
  const alignDone = async (changed, failed) => {
    setAlign(null);
    await reload();
    say(failed.length ? failed.join("; ") : T("Updated {n} references", { n: changed }));
  };
  const groupOp = async (name, args) => {
    const r = await call(name, args);
    if (r.ok) await refreshGroups();
    return r;
  };

  return (
    <div className="st-overlay" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="st-modal lb-modal" role="dialog" aria-label={T("Librarian")} ref={rootRef} tabIndex={-1}>
        <div className="st-modal-head" {...wc.headProps}>
          {wc.lights}
          <span className="st-modal-title">{T("Librarian")} · {project}</span>
          <LibHead project={project} styles={styles} styleId={styleId} selectedCount={selection.ids.length} shownCount={shown.length}
            onNew={addReference} onImport={() => setImporting(true)} onPdf={importPdf} onAlign={startAlign} onCopy={copyAs} onPickStyle={pickStyle} onAddStyle={addStyle}
            onCiteDraft={citeDraft} onCheckDraft={checkDraft} onExport={exportAs} onTrash={() => (trash ? setTrash(null) : openTrash())} onConnect={connectAgents} />
        </div>
        {notice && <div className="lb-notice" role="status" onClick={() => setNotice("")}>{notice}</div>}
        {importing && <LibImport call={call} onFile={importFile} onDone={importDone} onClose={() => setImporting(false)} />}
        {align && <LibAlign targets={align} call={call} onDone={alignDone} onClose={() => setAlign(null)} />}
        {trash && <LibTrash items={trash} onRestore={restore} onClose={() => setTrash(null)} />}
        {report && <LibReport report={report} onClose={() => setReport(null)} />}
        {rowDrag.drag && (
          <div className="lb-ghost" style={{ left: rowDrag.drag.x + 12, top: rowDrag.drag.y + 8 }} aria-hidden="true">
            {rowDrag.drag.ids.length > 1 ? T("{n} selected", { n: rowDrag.drag.ids.length }) : L.display(entries.find(e => e.id === rowDrag.drag.ids[0]) ?? {}).title}
          </div>
        )}
        {dropping && <div className="lb-drop" aria-hidden="true">{T("Drop PDFs to add them")}</div>}
        <div className="lb-body">
          <LibSidebar dropGroup={rowDrag.drag?.over} total={entries.length} smart={smart} groups={groups} tags={tags} scope={scope} onScope={setScope}
            onGroupAdd={name => groupOp("group_add", { name })}
            onGroupRename={(id, name) => groupOp("group_rename", { id, name })}
            onGroupRemove={async id => { await groupOp("group_remove", { id }); if (scope.group === id) setScope({ group: null, tag: null }); await reload(); }} />
          <LibList entries={entries} shown={loaded ? shown : []} selection={selection.ids} onRowDown={(ev, id) => rowDrag.begin(ev, dragIds(id))} wasDrag={rowDrag.wasDrag} onClick={clickRow} onStep={stepRow} onAll={selectAll}
            query={query} onQuery={setQuery} sort={sort} onSort={setSort} />
          {draft
            ? <LibDetail key={`draft:${draft.hash ?? "new"}`} apiRef={detail} entry={L.draftEntry(draft)} groups={groups} draft={draft}
                waiting={waiting} onSkipWaiting={skipWaiting}
                onSave={save} onLookup={lookup} onAttachDuplicate={attachDuplicate} onDiscardDraft={() => setDraft(null)} />
            : selected
              ? <LibDetail key={`${selected.id}:${selected.updatedAt}:${selected.attachments.length}:${selected.suggestions.length}`} apiRef={detail} entry={selected} groups={groups}
                  onSave={save} onMember={member} onDelete={remove} onCite={cite} onLookup={lookup} onReadPageOne={readPageOne} onAttach={attachPick} onRename={rename}
                  onOpenAttachment={openAttachment} onReadAttachment={readPdf} onLocateAttachment={locateAttachment} onRemoveAttachment={removeAttachment}
                  onSuggestion={resolveSuggestion} onOpenLink={url => bridge.openExternalUrl(url)} onCopyKey={copyKey} />
              : selectedMany.length > 1
                ? <LibBatch key={selection.ids.join(",")} entries={selectedMany} groups={groups} onGroup={batch.group} onTag={batch.tag}
                    onAlign={startAlign} onMerge={merge} onRemove={batch.remove} />
                : <section className="lb-detail lb-detail-empty">{T("Select a reference. Cmd-click or Shift-click selects several; drop PDFs here to add them.")}</section>}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { LibrarianWindow });
