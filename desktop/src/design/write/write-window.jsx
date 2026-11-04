/* write/write-window.jsx — the paper of an Academic project, in its own window (panel kind "write"). It
   only manages: it shows the pipeline Draft → Compile → Link, starts the steps and opens the files in VS
   Code or Typora, where the writing happens. All of it is Rust's (writing/commands.rs). A Compile goes on
   with the window closed; reopened, the window finds it and follows it. */

function WritePipeline({ status }) {
  const T = window.OMP_I18N.t;
  const W = window.OMP_WRITE;
  const detail = s => {
    if (s.id === "draft") return T("{n} files · {w} words", { n: s.total, w: s.words });
    if (s.id === "compile") return s.state === "active" ? `${s.done} / ${s.total} · ${s.current}` : T("{n} of {m} compiled", { n: s.done, m: s.total });
    return T(s.link === "fresh" ? "Up to date" : s.link === "stale" ? "Older than the LaTeX" : "Not linked yet");
  };
  return (
    <ol className="wr-pipe" aria-label={T("Pipeline")}>
      {W.stages(status).map(s => (
        <li key={s.id} className={`wr-stage ${s.state}`}>
          <span className="wr-stage-name">{T(s.label)}</span>
          <span className="wr-bar" role="progressbar" aria-valuenow={s.percent} aria-valuemin={0} aria-valuemax={100} aria-label={T(s.label)}>
            <span className="wr-bar-fill" style={{ width: `${s.percent}%` }} />
          </span>
          <span className="wr-stage-detail">{detail(s)}</span>
        </li>
      ))}
    </ol>
  );
}

function WriteWindow({ bridge, project, profile, onClose }) {
  const T = window.OMP_I18N.t;
  const W = window.OMP_WRITE;
  const [status, setStatus] = React.useState(null);
  const [notice, setNotice] = React.useState("");
  const [busy, setBusy] = React.useState("");   // "" | init | compile | link
  const [adding, setAdding] = React.useState(null);
  const rootRef = React.useRef(null);
  window.useModalFocus(rootRef);
  const wc = window.useWindowChrome(onClose, rootRef);
  const call = React.useCallback((name, args) => bridge.write(name, project, args), [bridge, project]);
  const load = React.useCallback(async () => {
    const res = await call("status");
    if (res.ok) setStatus(res.value); else setNotice(res.error);
  }, [call]);
  React.useEffect(() => {
    load();
    window.addEventListener("focus", load);   // the writing happens in another application
    return () => window.removeEventListener("focus", load);
  }, [load]);
  const running = Boolean(status?.run);
  React.useEffect(() => {
    if (!running) return undefined;
    const timer = setInterval(load, 1000);
    return () => clearInterval(timer);
  }, [running, load]);

  const step = async (name, args, say) => {
    setBusy(name); setNotice("");
    const pending = call(name, args);
    if (name === "compile") setTimeout(load, 300);
    const res = await pending;
    setBusy("");
    if (!res.ok) { setNotice(res.error); return load(); }
    if (say) setNotice(say(res.value));
    if (res.value?.ready !== undefined) return setStatus(res.value);
    return load();
  };
  const compiled = r => {
    const done = r.stopped ? T("Stopped after {n}", { n: r.compiled }) : T("Compiled {n}", { n: r.compiled });
    return r.missing.length ? `${done}. ${T("Not in the library: {keys}", { keys: r.missing.join(", ") })}` : done;
  };
  const compile = everything => step("compile", { everything, profile: profile || null }, compiled);
  const open = async (app, file) => {
    const res = await call("open", { app, file: file ?? null });
    if (!res.ok) setNotice(res.error);
  };
  const addSection = async e => {
    e.preventDefault();
    const name = adding.trim();
    if (!name) return setAdding(null);
    setAdding(null);
    return step("add_section", { name });
  };

  const next = W.nextStep(status);
  const working = running || busy !== "";
  const STATES = { fresh: "Compiled", stale: "Changed", new: "New" };
  return (
    <div className="st-overlay">
      <div className="st-modal wr-modal" role="dialog" aria-label={T("Write")} ref={rootRef} tabIndex={-1}>
        <div className="st-modal-head" {...wc.headProps}>
          {wc.lights}
          <span className="st-modal-title">{T("Write")} · {project}</span>
          <span className="lb-head-tools">
            <button type="button" className="btn ghost" disabled={!status?.ready} onClick={() => open("vscode")}>{T("Open in VS Code")}</button>
            <button type="button" className="btn ghost" disabled={!status?.ready} onClick={() => open("typora")}>{T("Open in Typora")}</button>
            <button type="button" className="btn ghost" disabled={!status?.ready} onClick={() => open("finder")}>{T("Show in Finder")}</button>
          </span>
        </div>
        {notice && <div className="lb-notice" role="status" onClick={() => setNotice("")}>{notice}</div>}
        {status && !status.ready ? (
          <div className="wr-empty">
            <p>{T("The paper lives in write/, a Git repository of its own: every step is committed, so nothing is lost and each Compile can be compared with its draft.")}</p>
            <button type="button" className="btn outlined accent" disabled={working} onClick={() => step("init")}>{T("Set up write/")}</button>
          </div>
        ) : status && (
          <div className="wr-body">
            <WritePipeline status={status} />
            <div className="wr-actions">
              <button type="button" className={`btn ${next === "compile" ? "outlined accent" : "ghost"}`} disabled={working} onClick={() => compile(false)}>{T("Compile")}</button>
              <button type="button" className={`btn ${next === "link" ? "outlined accent" : "ghost"}`} disabled={working || status.sections.some(s => s.state !== "fresh")} onClick={() => step("link")}>{T("Link")}</button>
              {running
                ? <button type="button" className="btn ghost" onClick={() => call("cancel")}>{T("Stop after this file")}</button>
                : <button type="button" className="btn ghost" disabled={working} title={T("Compiles every file again, changed or not")} onClick={() => compile(true)}>{T("Compile all")}</button>}
              <span className="wr-gap" />
              {status.main && <button type="button" className="btn ghost" onClick={() => open("vscode", "output/main.tex")}>{T("Open main.tex")}</button>}
              <span className="wr-git mono" title={T("The Git repository of write/")}>
                {status.repo?.branch ?? "—"} {status.repo?.head ?? ""} · {status.repo?.changed ? T("{n} changed", { n: status.repo.changed }) : T("clean")}
              </span>
            </div>
            {status.missing.length > 0 && (
              <div className="lb-error" role="alert">{T("Cited but not in the library: {keys}", { keys: status.missing.join(", ") })}</div>
            )}
            <ul className="wr-files">
              {status.sections.map(s => (
                <li key={s.file}>
                  <div className="wr-file">
                    <span className="wr-file-title">{s.title}</span>
                    <span className="wr-file-name mono">{s.file} · {T("{n} words", { n: s.words })}</span>
                  </div>
                  <span className={`wr-chip ${s.state}`}>{T(STATES[s.state])}</span>
                  <button type="button" className="btn ghost" onClick={() => open("typora", `markdown/${s.file}`)}>Typora</button>
                  <button type="button" className="btn ghost" onClick={() => open("vscode", `markdown/${s.file}`)}>VS Code</button>
                  {s.warnings.map(w => <div key={w} className="wr-warn" role="alert">{w}</div>)}
                </li>
              ))}
            </ul>
            {adding === null
              ? <button type="button" className="btn ghost wr-add" onClick={() => setAdding("")}>{T("Add a section…")}</button>
              : <form className="wr-add" onSubmit={addSection}><input className="lb-input" autoFocus value={adding} placeholder={T("Section name")} aria-label={T("Section name")} onChange={e => setAdding(e.target.value)} onBlur={() => { if (!adding.trim()) setAdding(null); }} /></form>}
          </div>
        )}
      </div>
    </div>
  );
}

Object.assign(window, { WriteWindow });
