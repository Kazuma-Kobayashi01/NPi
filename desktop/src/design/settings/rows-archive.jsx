/* design/settings/rows-archive.jsx — Settings → Chat's custom row `archive`
   (window.OMP_SETTINGS_ROWS): the conversations archived from the sidebar
   (the `archivedSessions` tweak holds their session files). Restore puts one
   back in the list; Delete removes its file for good, after a second click. */

function _SRAR_Archive({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const files = ctx.t.archivedSessions ?? [];
  const [saved, setSaved] = React.useState(null);
  const [asking, setAsking] = React.useState("");
  const [error, setError] = React.useState("");
  React.useEffect(() => {
    let live = true;
    Promise.resolve(ctx.bridge?.listSavedSessions?.()).then(l => { if (live) setSaved(Array.isArray(l) ? l : []); });
    return () => { live = false; };
  }, [ctx.bridge, files.length]);
  const byPath = new Map((saved ?? []).map(s => [s.path, s]));
  const restore = (f) => ctx.setTweak("archivedSessions", files.filter(x => x !== f));
  const remove = async (f) => {
    if (asking !== f) { setAsking(f); return; }
    setAsking(""); setError("");
    const r = byPath.has(f) ? await ctx.bridge?.deleteSavedSession?.(f) : { ok: true };
    if (r && !r.ok) { setError(String(r.error ?? "could not delete")); return; }
    restore(f);
  };
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-archive">
        {files.length === 0 && <div className="st-fav-empty">Nothing is archived.</div>}
        {files.map(f => {
          const s = byPath.get(f);
          const title = s?.title || f.split("/").pop();
          return (
            <div key={f} className="st-archive-row">
              <span className="st-archive-title" title={f}>{title}</span>
              <span className="st-archive-meta">{s ? (s.cwd || "").split("/").pop() : saved ? "file missing" : ""}</span>
              <Button onClick={() => restore(f)}>Restore</Button>
              <Button onClick={() => remove(f)}>{asking === f ? "Delete permanently?" : "Delete"}</Button>
            </div>
          );
        })}
        {error && <div className="st-row-error">{error}</div>}
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { archive: _SRAR_Archive });
