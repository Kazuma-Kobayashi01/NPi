/* chat/ask-subagent-form.jsx — omp's subagent preferences ask (Auto mode:
   which model and thinking level a batch uses) as one compact form: a
   searchable model list and a row of level pills, instead of two stacked
   radio lists. It edits the same draft the generic dialog does and sends
   through the same `answers()`, so omp gets the reply it always did.
   Detection and ordering rules: app/ask-dialog.js (`subagentForm`). */

// The UI language (app/i18n.js).
const _ASF_T = (key, vars) => window.OMP_I18N.t(key, vars);
const _ASF = window.OMP_ASK_DIALOG;

function AskSubagentForm({ questions, form, draft, onDraft, onSend, onCancel }) {
  const mq = questions[form.model];
  const tq = questions[form.thinking];
  const me = draft[form.model];
  const te = draft[form.thinking];
  const last = React.useMemo(() => _ASF.readSubagentMemory(), []);
  const order = React.useMemo(() => _ASF.modelOrder(mq, last?.model), [mq, last]);
  const [query, setQuery] = React.useState(null);   // null: showing the pick, not editing
  const [open, setOpen] = React.useState(false);
  const [active, setActive] = React.useState(0);
  const listRef = React.useRef(null);
  const typed = me.text.trim();
  const pickedLabel = me.picks.length ? mq.options[me.picks[0]].label : "";
  const shown = _ASF.filterModels(mq, order, query ?? "");
  const exact = query !== null && mq.options.some(o => o.label.toLowerCase() === query.trim().toLowerCase());
  // Rows: the matching models, then "use what I typed" when it is no model's name.
  const rows = [...shown.map(i => ({ i })), ...(query !== null && query.trim() && !exact ? [{ custom: query.trim() }] : [])];
  const canSend = !!typed || me.picks.length > 0 || !!(query ?? "").trim();

  React.useEffect(() => { setActive(0); }, [query]);
  React.useEffect(() => {
    if (open) listRef.current?.querySelector(".active")?.scrollIntoView?.({ block: "nearest" });
  }, [open, active]);

  const chooseRow = (row) => {
    if (!row) return;
    onDraft(row.custom !== undefined ? _ASF.type(draft, questions, form.model, row.custom) : _ASF.pick(draft, questions, form.model, row.i));
    setQuery(null);
    setOpen(false);
  };
  // What is typed in the model box counts even when it was never picked from the list.
  const resolved = () => _ASF.applyModelQuery(questions, form, draft, query);
  const send = () => { const d = resolved(); if (d[form.model].text.trim() || d[form.model].picks.length) onSend(d); };
  const stop = (e) => { e.preventDefault(); e.stopPropagation(); };

  const onModelKey = (e) => {
    if (e.nativeEvent.isComposing) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      stop(e);
      if (!open) { setOpen(true); return; }
      setActive(a => Math.max(0, Math.min(rows.length - 1, a + (e.key === "ArrowDown" ? 1 : -1))));
    } else if (e.key === "Enter") {
      stop(e);
      if (open && rows[active]) chooseRow(rows[active]); else send();
    } else if (e.key === "Escape") {
      stop(e);
      if (open) { setOpen(false); setQuery(null); } else onCancel();
    }
  };
  const onPillKey = (e) => {
    const n = tq.options.length;
    const at = te.picks[0] ?? 0;
    let to = null;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") to = (at + 1) % n;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") to = (at + n - 1) % n;
    else if (e.key === "Home") to = 0;
    else if (e.key === "End") to = n - 1;
    else if (e.key === "Escape") { stop(e); onCancel(); return; }
    else if (e.key === "Enter" && !e.nativeEvent.isComposing) { stop(e); send(); return; }
    if (to === null) return;
    stop(e);
    onDraft(_ASF.pick(draft, questions, form.thinking, to));
    e.currentTarget.parentElement.children[to]?.focus();
  };

  const inputValue = query ?? (typed || pickedLabel);
  const level = te.picks.length ? tq.options[te.picks[0]].label : "";
  return (
    <div className="ask-dialog ask-sub" onKeyDown={e => { if (e.key === "Tab" && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) e.stopPropagation(); }}>
      <div className="ask-sub-body">
        <div className="ask-sub-title">{_ASF_T("Subagent setup")}</div>
        <div className="ask-sub-sub">{_ASF_T("Choose the model and thinking level for this batch.")}</div>

        <label className="ask-sub-label" htmlFor="ask-sub-model">{_ASF_T("Model")}</label>
        <div className="ask-sub-combo">
          <div className="ask-sub-field">
          <input id="ask-sub-model" className="ask-sub-input" type="text" role="combobox" autoComplete="off" spellCheck={false}
            aria-expanded={open} aria-controls="ask-sub-list" aria-autocomplete="list"
            aria-activedescendant={open && rows[active] ? `ask-sub-opt-${active}` : undefined}
            value={inputValue} placeholder={_ASF_T("Search or type a model…")}
            onFocus={e => { setOpen(true); e.target.select(); }}
            onBlur={() => { onDraft(resolved()); setOpen(false); setQuery(null); }}
            onChange={e => { setQuery(e.target.value); setOpen(true); }}
            onKeyDown={onModelKey} />
          <button type="button" className="ask-sub-caret" tabIndex={-1} aria-label={_ASF_T(open ? "Hide models" : "Show models")}
            onMouseDown={e => e.preventDefault()} onClick={() => { setOpen(o => !o); document.getElementById("ask-sub-model")?.focus(); }}>
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"><path d="M3 4.5 6 7.5 9 4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" /></svg>
          </button>
          </div>
          {open && (
            <ul id="ask-sub-list" className="ask-sub-list" role="listbox" ref={listRef} aria-label={_ASF_T("Models")}>
              {rows.length === 0 && <li className="ask-sub-none">{_ASF_T("No model matches")}</li>}
              {rows.map((row, ri) => row.custom !== undefined ? (
                <li key="custom" id={`ask-sub-opt-${ri}`} role="option" aria-selected={false}
                  className={`ask-sub-opt custom${ri === active ? " active" : ""}`}
                  onMouseDown={e => e.preventDefault()} onClick={() => chooseRow(row)} onMouseEnter={() => setActive(ri)}>
                  Use “{row.custom}”
                </li>
              ) : (
                <li key={row.i} id={`ask-sub-opt-${ri}`} role="option" aria-selected={me.picks[0] === row.i && !typed}
                  className={`ask-sub-opt${ri === active ? " active" : ""}${me.picks[0] === row.i && !typed ? " selected" : ""}`}
                  onMouseDown={e => e.preventDefault()} onClick={() => chooseRow(row)} onMouseEnter={() => setActive(ri)}>
                  <span className="ask-sub-opt-label">{mq.options[row.i].label}</span>
                  {mq.recommended === row.i && <span className="chip warn">{_ASF_T("current")}</span>}
                  {mq.options[row.i].description && <span className="ask-sub-opt-desc">{mq.options[row.i].description}</span>}
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="ask-sub-label" id="ask-sub-think">{_ASF_T("Thinking")}</div>
        <div className="ask-sub-pills" role="radiogroup" aria-labelledby="ask-sub-think">
          {tq.options.map((o, oi) => {
            const on = te.picks[0] === oi;
            return (
              <button key={oi} type="button" role="radio" aria-checked={on} tabIndex={on || (te.picks.length === 0 && oi === 0) ? 0 : -1}
                className={`ask-sub-pill${on ? " on" : ""}`} onKeyDown={onPillKey}
                onClick={() => onDraft(_ASF.pick(draft, questions, form.thinking, oi))}>
                {o.label}
              </button>
            );
          })}
        </div>
      </div>
      <div className="ask-dialog-foot">
        <span className={`ask-progress${canSend ? "" : " incomplete"}`}>{canSend ? `${typed || pickedLabel} · ${level}` : _ASF_T("Choose a model")}</span>
        <span className="ask-foot-spacer" />
        <button type="button" className="ask-opt" onClick={onCancel}>{_ASF_T("Cancel")}</button>
        <button type="button" className="ask-submit" disabled={!canSend} onClick={send}>{_ASF_T("Start agents")}</button>
      </div>
    </div>
  );
}

Object.assign(window, { AskSubagentForm });
