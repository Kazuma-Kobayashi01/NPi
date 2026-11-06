/* design/settings/rows-memory.jsx — Settings → Memory's custom row `memory` (window.OMP_SETTINGS_ROWS): the
   switch, and every note the model keeps, by kind, to edit or delete. All of it is Rust's
   (src-tauri/src/memory/), how full memory is, the profile of each mode (USER-<MODE>.md), and
   whether Code and Academic sessions write skills (omp's autolearn, pinned per session by app_mode.rs
   `session_policy`). A change reaches a session when it next starts. */

function _SRMem_Note({ note, onSave, onDelete }) {
  const T = key => window.OMP_I18N.t(key);
  const M = window.OMP_MEMORY;
  const [draft, setDraft] = React.useState(null);
  const [armed, setArmed] = React.useState(false);
  const age = M.age(note.updatedAt, Date.now() / 1000);
  const when = window.OMP_I18N.t(age.key, { n: age.n });
  const save = async () => { if (await onSave(note.id, draft)) setDraft(null); };
  return (
    <li className="st-mem-note">
      <span className="st-mem-title">{note.title}</span>
      {draft === null
        ? <button type="button" className="st-mem-text selectable" title={T("Edit")} onClick={() => setDraft(note.text)}>{note.text}</button>
        : <textarea className="st-textarea selectable" rows={2} autoFocus maxLength={300} value={draft} aria-label={note.title} onChange={e => setDraft(e.target.value)}
            onKeyDown={e => { if (e.key === "Escape") { e.stopPropagation(); setDraft(null); } if (window.isSubmitEnter(e)) { e.preventDefault(); save(); } }} onBlur={save} />}
      <span className="st-mem-when">{when}</span>
      <button type="button" className={`btn ghost${armed ? " armed" : ""}`} onClick={() => { if (armed) onDelete(note.id); else setArmed(true); }} onBlur={() => setArmed(false)}>{armed ? T("Delete?") : T("Delete")}</button>
    </li>
  );
}

/** One mode's profile (USER-<MODE>.md): the model's summary of how the user likes to work there. */
function _SRMem_Profile({ mode, label, text, limit, onSave }) {
  const T = key => window.OMP_I18N.t(key);
  const [draft, setDraft] = React.useState(text);
  React.useEffect(() => setDraft(text), [text]);
  const count = [...draft].length;
  return (
    <div className="st-mem-profile">
      <div className="st-mem-profile-head">
        <h4 className="st-mem-kind">{T(label)}</h4>
        <span className={`st-mem-when${count > limit ? " over" : ""}`}>{window.OMP_MEMORY.share(count, limit)}</span>
      </div>
      <textarea className="st-textarea selectable" rows={3} value={draft} aria-label={T(label)}
        placeholder={T("Empty. The assistant fills this in as it learns how you like to work here.")}
        onChange={e => setDraft(e.target.value)} onBlur={() => { if (draft !== text) onSave(mode, draft); }} />
    </div>
  );
}

const _SRMem_PROFILES = [["chat", "How you work in Chat"], ["code", "How you work in Code"], ["academic", "How you work in Academic"]];

function _SRMem_Memory({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsToggle: Toggle, SettingsButton: Button } = window;
  const T = key => window.OMP_I18N.t(key);
  const [state, setState] = React.useState(null);
  const [error, setError] = React.useState("");
  const [armed, setArmed] = React.useState(false);
  const call = React.useCallback(async (name, args) => {
    const res = await ctx.bridge.memory(name, args);
    if (res.ok) { setState(res.value); setError(""); } else setError(res.error);
    return res.ok;
  }, [ctx.bridge]);
  React.useEffect(() => { call("state"); }, [call]);
  const groups = window.OMP_MEMORY.grouped(state?.notes);
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-mem">
        <Toggle value={state?.enabled ?? true} label={row.label} onChange={on => call("set_enabled", { on })} />
        {state?.enabled && (
          <div className="st-mem-sub">
            <Toggle value={state.skillLearning} label={T("Write skills from what worked")} onChange={on => call("set_skill_learning", { on })} />
            <span><b>{T("Write skills from what worked")}</b> {T("In Code and Academic, after a task that took many steps, the agent may save the steps as a skill for next time. Each new skill is shown with Undo; all of them are in Settings → Skills. Chat never does.")}</span>
          </div>
        )}
        {state?.usage && (
          <div className="st-mem-usage">
            <span>{T("Notes")} {window.OMP_MEMORY.share(state.usage.notes, state.usage.maxNotes)}</span>
            <span>{T("In each prompt")} {window.OMP_MEMORY.share(state.usage.promptChars, state.usage.maxPromptChars)} {T("characters")}</span>
            {state.usage.leftOut > 0 && <span>{T("{n} older notes left out of the prompt; the assistant can still look them up.").replace("{n}", state.usage.leftOut)}</span>}
          </div>
        )}
        {state && _SRMem_PROFILES.map(([mode, label]) => (
          <_SRMem_Profile key={mode} mode={mode} label={label} text={state.profiles?.[mode] ?? ""} limit={state.profileLimit}
            onSave={(m, text) => call("set_profile", { mode: m, text })} />
        ))}
        {error && <div className="lb-error" role="alert">{error}</div>}
        {state && groups.length === 0 && <div className="st-fav-empty">{T("Nothing yet. Notes appear here as you talk.")}</div>}
        {groups.map(g => (
          <section key={g.kind}>
            <h4 className="st-mem-kind">{T(g.label)}</h4>
            <ul className="st-mem-list">
              {g.notes.map(n => <_SRMem_Note key={`${n.id}:${n.updatedAt}`} note={n} onSave={(id, text) => call("update", { id, text })} onDelete={id => call("delete", { id })} />)}
            </ul>
          </section>
        ))}
        <div className="st-chattext-foot">
          {state?.notes.length > 0 && <Button onClick={() => { if (armed) { setArmed(false); call("clear"); } else setArmed(true); }}>{armed ? T("Delete every note?") : T("Delete all")}</Button>}
          {state && <span className="st-fav-empty selectable">{state.path}</span>}
        </div>
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { memory: _SRMem_Memory });
