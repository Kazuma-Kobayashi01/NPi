/* librarian/lib-align.jsx — "Align metadata": the references asked for are compared with Crossref (then OpenAlex) one
   by one, no model involved, and the differences are listed for the user to accept. Applying fills
   blank fields; "also replace differing values" overwrites. A record with another title is reported
   and never offered. Only the DOI, or the title and first author, is sent (librarian/tools.rs). */

const _LA_FIELDS = { "container-title": "Journal / book / conference", DOI: "DOI", URL: "URL", volume: "Volume", issue: "Issue", page: "Pages", issued: "Year", publisher: "Publisher", abstract: "Abstract", author: "Authors", type: "Type" };
const _LA_STATUS = { would_change: "differs", applied: "applied", up_to_date: "up to date", mismatch: "another paper", no_record: "no record" };

function LibAlign({ targets, call, onDone, onClose }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const [rows, setRows] = React.useState([]);       // { entry, status, reason, changes, on }
  const [running, setRunning] = React.useState(true);
  const [overwrite, setOverwrite] = React.useState(false);
  const [applying, setApplying] = React.useState(false);
  const stop = React.useRef(false);

  const preview = React.useCallback(async replace => {
    stop.current = false;
    setRunning(true); setRows([]);
    for (const entry of targets) {
      if (stop.current) break;
      const res = await call("align", { citekey: entry.citekey, apply: false, overwrite: replace });
      const r = res.ok ? res.value.results[0] : { status: "no_record", reason: res.error };
      setRows(list => [...list, { entry, status: r.status, reason: r.reason ?? "", changes: r.changes ?? [], on: r.status === "would_change" }]);
    }
    setRunning(false);
  }, [targets, call]);
  React.useEffect(() => { preview(false); return () => { stop.current = true; }; }, [preview]);

  const chosen = rows.filter(r => r.on && r.status === "would_change");
  const apply = async () => {
    setApplying(true);
    let changed = 0;
    const failed = [];
    for (const row of chosen) {
      const res = await call("align", { citekey: row.entry.citekey, apply: true, overwrite });
      if (res.ok && res.value.referencesChanged > 0) changed += 1;
      else if (!res.ok) failed.push(`${row.entry.citekey}: ${res.error}`);
    }
    setApplying(false);
    onDone(changed, failed);
  };
  const show = v => {
    if (v === null || v === undefined || v === "") return "—";
    if (typeof v === "string" || typeof v === "number") return String(v);
    if (Array.isArray(v)) return v.map(x => (typeof x === "object" && x ? L.personText(x) || JSON.stringify(x) : String(x))).join("; ");
    if (v["date-parts"]) return String(v["date-parts"][0]?.join("-") ?? "");
    return JSON.stringify(v);
  };
  const toggle = id => setRows(list => list.map(r => (r.entry.id === id ? { ...r, on: !r.on } : r)));

  return (
    <div className="lb-report lb-align" role="region" aria-label={T("Align metadata")}>
      <div className="lb-report-head">
        <strong>{T("Align metadata")}</strong>
        <span>{running
          ? T("Checking {done} of {n}…", { done: rows.length, n: targets.length })
          : T("{n} checked, {m} with differences", { n: rows.length, m: rows.filter(r => r.status === "would_change").length })}</span>
        <label className="lb-check">
          <input type="checkbox" checked={overwrite} disabled={running || applying}
            onChange={e => { setOverwrite(e.target.checked); preview(e.target.checked); }} />
          {T("Also replace differing values")}
        </label>
        {running
          ? <button type="button" className="btn ghost" onClick={() => { stop.current = true; }}>{T("Stop")}</button>
          : <button type="button" className="btn outlined accent" disabled={chosen.length === 0 || applying} onClick={apply}>
              {applying ? T("Applying…") : T("Apply to {n}", { n: chosen.length })}
            </button>}
        <button type="button" className="btn ghost" onClick={onClose}>{T("Close")}</button>
      </div>
      <ul className="lb-align-list">
        {rows.map(r => (
          <li key={r.entry.id} className={`lb-align-row ${r.status}`}>
            <label className="lb-align-head">
              <input type="checkbox" checked={r.on} disabled={r.status !== "would_change"} onChange={() => toggle(r.entry.id)} />
              <span className="lb-align-name">{L.display(r.entry).title}</span>
              <span className={`lb-mark${r.status === "mismatch" || r.status === "no_record" ? " warn" : ""}`}>{T(_LA_STATUS[r.status] ?? r.status)}</span>
            </label>
            {r.reason && <small className="lb-state">{r.reason}</small>}
            {r.changes.length > 0 && (
              <table className="lb-lookup-table"><tbody>
                {r.changes.map(c => (
                  <tr key={c.field} className="differs"><th>{T(_LA_FIELDS[c.field] ?? c.field)}</th>
                    <td>{show(c.from) !== "—" && <del>{show(c.from)}</del>}{" "}{show(c.to)}</td></tr>
                ))}
              </tbody></table>
            )}
          </li>
        ))}
      </ul>
      <small>{T("Only the DOI, or the title and first author, is sent to Crossref and OpenAlex. Blank fields are filled; your own values stay unless you tick the box above.")}</small>
    </div>
  );
}

Object.assign(window, { LibAlign });
