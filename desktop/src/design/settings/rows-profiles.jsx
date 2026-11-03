/* ═════════════════════════════════════════════════════════════════════
   design/settings/rows-profiles.jsx — custom Settings row of Agent
   (window.OMP_SETTINGS_ROWS):
     profileList  every omp profile (`omp --profile <id>`: its own auth,
                  sessions, settings and caches): which one new tabs start
                  in, which one the active tab runs, new, rename, remove.
                  It replaced the top-bar menu; the handlers are the same
                  (ctx.onSelectProfile / onCreateProfile / onRenameProfile /
                  onDeleteProfile / onSetStartupProfile).
   ═════════════════════════════════════════════════════════════════════ */

function _SRP_ProfileList({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button, DEFAULT_PROFILE_ID, Icon } = window;
  const T = window.OMP_I18N.t;
  const [error, setError] = React.useState(null);
  const [edit, setEdit] = React.useState(null);        // { id | null (new), draft }
  const [armed, setArmed] = React.useState(null);      // profile id awaiting a second delete click
  const busy = React.useRef(false);

  // One action at a time; a refusal shows under the list.
  const run = async (work) => {
    if (busy.current) return;
    busy.current = true;
    setError(null);
    try {
      const res = await work();
      if (res && res.ok === false) setError(res.error ?? T("could not save"));
      return res;
    } catch (e) {
      setError(e?.message ?? String(e));
    } finally {
      busy.current = false;
    }
  };

  const commit = () => run(async () => {
    const name = edit.draft.trim();
    if (!name) return { ok: false, error: T("name required") };
    const res = edit.id ? await ctx.onRenameProfile(edit.id, name) : await ctx.onCreateProfile(name);
    if (res?.ok) setEdit(null);
    return res;
  });

  const remove = (id) => {
    if (armed !== id) { setArmed(id); return; }
    setArmed(null);
    return run(() => ctx.onDeleteProfile(id));
  };

  return (
    <Row label={row.label} desc={row.desc} query={query}>
      <div className="st-profiles">
        {ctx.profiles.map(p => {
          const startup = p.id === ctx.startupProfileId;
          const inUse = p.id === ctx.activeProfileId;
          return (
            <div key={p.id} className="st-profile">
              <button type="button" role="radio" aria-checked={startup} className={`st-profile-start${startup ? " on" : ""}`}
                title={T("New tabs and the next launch start in this profile")} onClick={() => !startup && run(() => ctx.onSetStartupProfile(p.id))}>
                {startup && <Icon name="check" size={10} color="var(--accent)" dotColor="var(--accent)" />}
              </button>
              {edit?.id === p.id ? (
                <input className="st-profile-input" autoFocus maxLength={48} value={edit.draft}
                  onChange={e => setEdit({ id: p.id, draft: e.target.value })}
                  onKeyDown={e => { if (window.isSubmitEnter(e)) { e.preventDefault(); commit(); } if (e.key === "Escape") { e.stopPropagation(); setEdit(null); } }} />
              ) : (
                <span className="st-profile-name">
                  <b>{p.name}</b>
                  <i className="mono">{p.id === DEFAULT_PROFILE_ID ? "~/.omp/agent" : `~/.omp/profiles/${p.id}`}</i>
                </span>
              )}
              <span className="st-btns">
                {edit?.id === p.id
                  ? <Button accent onClick={commit}>{T("Save")}</Button>
                  : <>
                      {inUse
                        ? <span className="st-profile-tag">{T("this tab")}</span>
                        : <Button title={T("Restarts this tab's agent; its conversation is dropped")}
                            onClick={() => run(() => ctx.onSelectProfile(p.id))}>{T("Use in this tab")}</Button>}
                      <Button onClick={() => setEdit({ id: p.id, draft: p.name })}>{T("Rename")}</Button>
                      {p.id !== DEFAULT_PROFILE_ID &&
                        <Button title={T("Only unlists it; the files stay on disk")} onClick={() => remove(p.id)}>{armed === p.id ? T("Delete?") : T("Delete")}</Button>}
                    </>}
              </span>
            </div>
          );
        })}
        <div className="st-profile st-profile-new">
          {edit && edit.id === null ? (
            <>
              <input className="st-profile-input" autoFocus maxLength={48} placeholder="work, home, client…" value={edit.draft}
                onChange={e => setEdit({ id: null, draft: e.target.value })}
                onKeyDown={e => { if (window.isSubmitEnter(e)) { e.preventDefault(); commit(); } if (e.key === "Escape") { e.stopPropagation(); setEdit(null); } }} />
              <span className="st-btns"><Button accent onClick={commit}>{T("Create")}</Button></span>
            </>
          ) : <Button onClick={() => setEdit({ id: null, draft: "" })}>{T("New profile…")}</Button>}
        </div>
        {error && <div className="st-row-error" role="alert">{error}</div>}
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { profileList: _SRP_ProfileList });
