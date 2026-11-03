/* librarian/lib-list.jsx — the middle column: search, sort and the rows. */

function LibList({ entries, shown, selectedId, onSelect, query, onQuery, sort, onSort }) {
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
      <div className="lb-rows" role="listbox" aria-label={T("References")}>
        {shown.map(e => {
          const d = L.display(e);
          return (
            <button key={e.id} type="button" role="option" aria-selected={e.id === selectedId}
              className={`lb-row${e.id === selectedId ? " on" : ""}`} onClick={() => onSelect(e.id)}>
              <span className="lb-row-title">{d.title}</span>
              <span className="lb-row-meta">
                {[d.authors, d.year, d.container].filter(Boolean).join(" · ")}
              </span>
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
      <div className="lb-list-foot mono">{T("{shown} of {total}", { shown: shown.length, total: entries.length })}</div>
    </section>
  );
}

Object.assign(window, { LibList });
