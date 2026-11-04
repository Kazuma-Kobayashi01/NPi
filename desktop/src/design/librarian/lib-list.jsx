/* librarian/lib-list.jsx — the middle column: search, sort and the rows. */

function LibList({ entries, shown, selection, onRowDown, wasDrag, onClick, onStep, onAll, query, onQuery, sort, onSort }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const { Icon } = window;
  const SORTS = { added: "Added", year: "Year", author: "Author", title: "Title" };
  return (
    <section className="lb-list" aria-label={T("References")}>
      <div className="lb-list-top">
        <label className="lb-search">
          <Icon name="search" size={12} color="var(--fg-4)" />
          <input value={query} placeholder={T("Search references")} aria-label={T("Search references")}
            onChange={e => onQuery(e.target.value)} />
        </label>
        <select className="lb-select" aria-label={T("Sort")} value={sort.key} onChange={e => onSort({ ...sort, key: e.target.value })}>
          {L.SORT_KEYS.map(k => <option key={k} value={k}>{T(SORTS[k])}</option>)}
        </select>
        <button type="button" className="btn icon ghost" title={sort.dir === "asc" ? T("Ascending") : T("Descending")}
          onClick={() => onSort({ ...sort, dir: sort.dir === "asc" ? "desc" : "asc" })}>
          <span className="mono">{sort.dir === "asc" ? "↑" : "↓"}</span>
        </button>
      </div>
      <div className="lb-rows" role="listbox" aria-multiselectable="true" aria-label={T("References")} tabIndex={0}
        onKeyDown={ev => {
          if (ev.key === "ArrowDown" || ev.key === "ArrowUp") { ev.preventDefault(); onStep(ev.key === "ArrowDown" ? 1 : -1, ev.shiftKey); }
          if ((ev.metaKey || ev.ctrlKey) && ev.key === "a") { ev.preventDefault(); onAll(); }
        }}>
        {shown.map(e => {
          const d = L.display(e);
          const on = selection.includes(e.id);
          return (
            <button key={e.id} type="button" role="option" aria-selected={on} data-ref={e.id}
              className={`lb-row${on ? " on" : ""}`} onPointerDown={ev => onRowDown(ev, e.id)}
              onClick={ev => { if (!wasDrag()) onClick(e.id, { toggle: ev.metaKey || ev.ctrlKey, range: ev.shiftKey }); }}>
              <span className="lb-row-title">{d.title}</span>
              <span className="lb-row-meta">
                {[d.authors, d.year, d.container].filter(Boolean).join(" · ")}
              </span>
              {((e.attachments ?? []).length > 0 || L.incomplete(e)) && (
                <span className="lb-row-marks">
                  {(e.attachments ?? []).length > 0 && <span className="lb-mark">PDF</span>}
                  {L.incomplete(e) && <span className="lb-mark warn">{T("incomplete")}</span>}
                </span>
              )}
            </button>
          );
        })}
        {shown.length === 0 && (
          <div className="lb-empty">
            {entries.length === 0
              ? T("No references yet. Import a .ris, .bib or .json file, or add one by hand.")
              : T("No reference matches.")}
          </div>
        )}
      </div>
      <div className="lb-list-foot mono">
        {T("{shown} of {total}", { shown: shown.length, total: entries.length })}
        {selection.length > 1 && ` · ${T("{n} selected", { n: selection.length })}`}
      </div>
    </section>
  );
}

Object.assign(window, { LibList });
