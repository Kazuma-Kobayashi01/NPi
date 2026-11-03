/* librarian/librarian-window.jsx — the Librarian of one Academic project:
   a floating window with groups and tags on the left, the searchable list in
   the middle and one reference's fields on the right (docs/PLAN_LIBRARIAN.md,
   L1). The data is Rust's (library_* commands through `bridge.library`);
   what is shown and how it is filtered is app/librarian.js. */

function LibrarianWindow({ bridge, project, onClose }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const { Icon } = window;
  const [entries, setEntries] = React.useState([]);
  const [groups, setGroups] = React.useState([]);
  const [loaded, setLoaded] = React.useState(false);
  const [scope, setScope] = React.useState({ group: null, tag: null });
  const [query, setQuery] = React.useState("");
  const [sort, setSort] = React.useState({ key: "added", dir: "desc" });
  const [selectedId, setSelectedId] = React.useState(null);
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

  const reload = React.useCallback(async () => {
    const [list, grp] = await Promise.all([call("list"), call("groups")]);
    if (list.ok) {
      setEntries(list.value);
      const waiting = list.value.reduce((n, e) => n + (e.suggestions?.length ?? 0), 0);
      if (waiting > 0) say(T("{n} suggestions from the Copilot are waiting", { n: waiting }));
    } else say(list.error);
    if (grp.ok) setGroups(grp.value);
    setLoaded(true);
  }, [call]);
  React.useEffect(() => { reload(); }, [reload]);
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
  /** Copies `text` (and `html` when the browser takes rich text) to the clipboard. */
  const copyRich = async (text, html) => {
    try {
      if (html && window.ClipboardItem) {
        await navigator.clipboard.write([new ClipboardItem({
          "text/plain": new Blob([text], { type: "text/plain" }),
          "text/html": new Blob([html], { type: "text/html" }),
        })]);
      } else await navigator.clipboard.writeText(text);
      return true;
    } catch { return false; }
  };
  /** `kind`: "citation" (in text) or "reference" (the list entry) for `ids`, in the chosen style. */
  const cite = async (ids, kind) => {
    const r = await call("format", { style: styleId, ids });
    if (!r.ok) { say(r.error); return false; }
    const lines = kind === "citation" ? r.value.citations : r.value.bibliography;
    if (lines.length === 0) { say(T("Nothing to copy")); return false; }
    const text = lines.map(l => l.text).join("\n");
    const html = lines.map(l => `<p>${l.html}</p>`).join("");
    if (await copyRich(text, html)) return true;
    say(T("could not copy"));
    return false;
  };

  React.useEffect(() => {
    const onKey = e => { if (e.key === "Escape") { e.preventDefault(); onClose(); } };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const replace = updated => setEntries(list => list.map(e => (e.id === updated.id ? updated : e)));
  const shown = React.useMemo(() => L.sorted(L.visible(entries, { ...scope, query }), sort.key, sort.dir), [entries, scope, query, sort]);
  const tags = React.useMemo(() => L.tagCounts(entries), [entries]);
  const selected = entries.find(e => e.id === selectedId) ?? null;

  const addReference = async () => {
    const res = await call("add", { csl: { type: "article-journal", title: T("New reference") } });
    if (!res.ok) return say(res.error);
    setEntries(list => [...list, res.value]);
    setScope({ group: null, tag: null }); setQuery(""); setSelectedId(res.value.id);
  };
  const importFile = async () => {
    const res = await call("import_pick");
    if (!res.ok) return say(res.error);
    if (res.value === null) return;
    say(T("Added {n}; {m} already in the library", { n: res.value.added.length, m: res.value.duplicates.length }));
    await reload();
  };
  const exportAs = async format => {
    const ids = shown.map(e => e.id);
    const res = await call("export_save", { format, ids });
    if (!res.ok) say(res.error); else if (res.value) say(T("Saved {n} references", { n: ids.length }));
  };
  const importPdf = async () => {
    const res = await call("pdf_pick");
    if (!res.ok) return say(res.error);
    if (!res.value) return;
    setSelectedId(null);
    setDraft(res.value);
  };
  const attachPick = async entry => {
    const res = await call("attach_pick", { id: entry.id });
    if (!res.ok) return say(res.error);
    if (res.value) { replace(res.value); say(T("Attached")); }
  };
  const attachDuplicate = async existing => {
    const res = await call("attach_pdf", { id: existing.id, hash: draft.hash });
    if (!res.ok) return say(res.error);
    replace(res.value); setDraft(null); setSelectedId(existing.id);
    say(T("Attached to {key}", { key: existing.citekey }));
  };
  const openAttachment = async a => {
    const res = await call("attachment_open", { id: a.id });
    if (!res.ok) say(res.error);
  };
  const removeAttachment = async a => {
    const res = await call("attachment_remove", { id: a.id });
    if (!res.ok) return say(res.error);
    await reload();
  };
  const lookup = args => call("lookup", args);
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
    setGroups((await call("groups")).value ?? groups);
    say(T("Restored {key}", { key: item.citekey }));
  };
  const resolveSuggestion = async (sg, accept) => {
    const r = await call("suggestion_resolve", { id: sg.id, accept });
    if (!r.ok) return say(r.error);
    replace(r.value);
    if (accept && sg.kind === "group") setGroups((await call("groups")).value ?? groups);
  };
  const checkDraft = async () => {
    const r = await call("check_draft_pick");
    if (!r.ok) return say(r.error);
    if (r.value) setReport(r.value);
  };
  const saveDraft = async csl => {
    const res = await call("add_pdf", { csl, hash: draft.hash });
    if (!res.ok) return res;
    if (res.value.duplicate) {
      setDraft(d => ({ ...d, duplicateOf: res.value.duplicate }));
      return { ok: false, error: T("Already in the library") };
    }
    setEntries(list => [...list, res.value.entry]);
    setScope({ group: null, tag: null }); setQuery("");
    setDraft(null); setSelectedId(res.value.entry.id);
    return { ok: true };
  };
  const save = async (entry, csl, tagList, notes) => {
    if (entry.draft) return saveDraft(csl);
    let current = entry;
    if (L.formChanged(entry.csl, L.toForm(csl))) {
      const r = await call("update", { id: entry.id, csl });
      if (!r.ok) return r;
      current = r.value;
    }
    const m = await call("meta", { id: entry.id, tags: tagList, notes });
    if (!m.ok) return m;
    replace(m.value);
    return { ok: true };
  };
  const member = async (entry, group, on) => {
    const r = await call("member", { id: entry.id, group, member: on });
    if (!r.ok) return say(r.error);
    replace(r.value);
    setGroups((await call("groups")).value ?? groups);
  };
  const remove = async entry => {
    const r = await call("remove", { id: entry.id });
    if (!r.ok) return say(r.error);
    setEntries(list => list.filter(e => e.id !== entry.id));
    setSelectedId(null);
    if (trash) setTrash(null);
    say(T("Moved {key} to the trash", { key: entry.citekey }));
    setGroups((await call("groups")).value ?? groups);
  };
  const groupOp = async (name, args) => {
    const r = await call(name, args);
    if (r.ok) { const g = await call("groups"); if (g.ok) setGroups(g.value); }
    return r;
  };

  return (
    <div className="st-overlay" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="st-modal lb-modal" role="dialog" aria-label={T("Librarian")} ref={rootRef} tabIndex={-1}>
        <div className="st-modal-head" {...wc.headProps}>
          {wc.lights}
          <span className="st-modal-title">{T("Librarian")} · {project}</span>
          <span className="lb-head-tools">
            <button type="button" className="btn ghost" onClick={addReference}><Icon name="plus" size={11} /> {T("New reference")}</button>
            <button type="button" className="btn ghost" onClick={importFile}>{T("Import…")}</button>
            <button type="button" className="btn ghost" onClick={importPdf}>{T("Add PDF…")}</button>
            <select className="lb-select" aria-label={T("Citation style")} value={styleId}
              onChange={e => { if (e.target.value === "__add") addStyle(); else pickStyle(e.target.value); }}>
              {styles.length === 0 && <option value={styleId}>{styleId}</option>}
              {styles.map(st => <option key={st.id} value={st.id}>{st.name}</option>)}
              <option value="__add">{T("Add a .csl style…")}</option>
            </select>
            <button type="button" className="btn ghost" disabled={shown.length === 0}
              title={T("Copies the reference list of what the list shows")}
              onClick={async () => { if (await cite(shown.map(e => e.id), "reference")) say(T("Copied {n} references", { n: shown.length })); }}>
              {T("Copy list")}
            </button>
            <button type="button" className="btn ghost" title={T("References you removed; restoring brings back their groups, tags, notes and files")} onClick={() => (trash ? setTrash(null) : openTrash())}>{T("Trash")}</button>
            <button type="button" className="btn ghost" title={T("Checks a draft's citations against the library")} onClick={checkDraft}>{T("Check a draft…")}</button>
            <button type="button" className="btn ghost" title={T("Replaces [@citekey] in a Markdown draft and appends the references")}
              onClick={async () => {
                const r = await call("cite_draft", { style: styleId });
                if (!r.ok) return say(r.error);
                if (!r.value) return;
                say(r.value.missing.length
                  ? T("Cited {n}; not in the library: {keys}", { n: r.value.cited, keys: r.value.missing.join(", ") })
                  : T("Cited {n} in the draft", { n: r.value.cited }));
              }}>{T("Cite a draft…")}</button>
            <select className="lb-select" aria-label={T("Export")} value="" disabled={shown.length === 0}
              onChange={e => { if (e.target.value) exportAs(e.target.value); }}>
              <option value="">{T("Export")}</option>
              <option value="bibtex">BibTeX (.bib)</option>
              <option value="ris">RIS (.ris)</option>
              <option value="csl-json">CSL-JSON (.json)</option>
            </select>
          </span>
        </div>
        {notice && <div className="lb-notice" role="status" onClick={() => setNotice("")}>{notice}</div>}
        {trash && (
          <div className="lb-report lb-trash" role="region" aria-label={T("Trash")}>
            <div className="lb-report-head">
              <strong>{T("Trash")}</strong>
              <span>{trash.length === 0 ? T("Nothing in the trash.") : T("{n} removed", { n: trash.length })}</span>
              <button type="button" className="btn ghost" onClick={() => setTrash(null)}>{T("Close")}</button>
            </div>
            <ul>
              {trash.map(item => (
                <li key={item.id}>
                  <button type="button" className="btn ghost" onClick={() => restore(item)}>{T("Restore")}</button>
                  <code>{item.citekey}</code> <span>{(item.csl?.title ?? "").slice(0, 100)}</span>
                </li>
              ))}
            </ul>
            <small>{T("Removed references are kept, and their citekeys are never reused.")}</small>
          </div>
        )}
        {report && (
          <div className="lb-report" role="region" aria-label={T("Citation check")}>
            <div className="lb-report-head">
              <strong>{T("Citation check")}</strong>
              <span>{report.findings.length === 0
                ? T("No checkable citations found.")
                : T("{matched} matched, {problems} need attention", { matched: report.matched, problems: report.problems })}</span>
              <button type="button" className="btn ghost" onClick={() => setReport(null)}>{T("Close")}</button>
            </div>
            <ul>
              {report.findings.map((f, i) => (
                <li key={i} className={f.status === "matched" ? "ok" : "check"}>
                  <span className="lb-report-status">{f.status === "matched" ? T("ok") : T("check")}</span>
                  <code>{f.text}</code> <span>{f.detail}</span>
                </li>
              ))}
            </ul>
            <small>{T("Only [@citekey], Author (Year) and DOIs are checked; numbered citations like [1] are not.")}</small>
          </div>
        )}
        <div className="lb-body">
          <LibSidebar total={entries.length} groups={groups} tags={tags} scope={scope} onScope={setScope}
            onGroupAdd={name => groupOp("group_add", { name })}
            onGroupRename={(id, name) => groupOp("group_rename", { id, name })}
            onGroupRemove={async id => { await groupOp("group_remove", { id }); if (scope.group === id) setScope({ group: null, tag: null }); await reload(); }} />
          <LibList entries={entries} shown={loaded ? shown : []} selectedId={selectedId} onSelect={id => { setDraft(null); setSelectedId(id); }}
            query={query} onQuery={setQuery} sort={sort} onSort={setSort} />
          {draft
            ? <LibDetail key={`draft:${draft.hash}`} entry={L.draftEntry(draft)} groups={groups} draft={draft}
                onSave={save} onLookup={lookup} onAttachDuplicate={attachDuplicate} onDiscardDraft={() => setDraft(null)} />
            : selected
              ? <LibDetail key={`${selected.id}:${selected.updatedAt}:${selected.attachments.length}:${selected.suggestions.length}`} entry={selected} groups={groups}
                  onSave={save} onMember={member} onDelete={remove} onCite={cite} onLookup={lookup} onAttach={attachPick}
                  onOpenAttachment={openAttachment} onRemoveAttachment={removeAttachment} onSuggestion={resolveSuggestion} />
              : <section className="lb-detail lb-detail-empty">{T("Select a reference.")}</section>}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { LibrarianWindow });
