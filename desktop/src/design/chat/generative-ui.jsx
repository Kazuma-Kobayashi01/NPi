// All model strings render as React text; no HTML, actions, URLs or code.
class GenerativeUiBoundary extends React.Component {
  constructor(props) { super(props); this.state = { error: null }; }
  static getDerivedStateFromError(error) { return { error: error.message }; }
  render() {
    if (this.state.error) return <section className="gui-card gui-failed" role="alert">UI unavailable: {this.state.error}</section>;
    return this.props.children;
  }
}

// Bounded per-document drafts survive tab unmounts, never cross task identities.
const generativeUiDrafts = new Map();
function generativeUiIdentity(msg) { return JSON.stringify([msg.session_id, msg.ui_id, msg.task_id, msg.message_id]); }
function generativeUiRemember(key, record) {
  generativeUiDrafts.delete(key);
  generativeUiDrafts.set(key, record);
  if (generativeUiDrafts.size > 128) generativeUiDrafts.delete(generativeUiDrafts.keys().next().value);
}

// A real component so its loader hook has a component boundary; the registry
// entry below only creates the element (never calls the hook directly).
function GenerativeUiImage({ component: c }) {
  const onLink = React.useContext(window.MarkdownLinkContext);
  const image = window.OMP_CHAT_IMAGE.useChatImage(c.src, { thumbnail: c.thumbnail });
  const source = c.sourceUrl && /^https:\/\//i.test(c.sourceUrl)
    ? <a href={c.sourceUrl} target="_blank" rel="noopener noreferrer" className="md-image-attribution"
      title={c.sourceUrl} onClick={e => { e.stopPropagation(); onLink?.(e); }}>
      {c.attribution || "Source"}
    </a>
    : null;
  const credit = source || (c.attribution && <span className="gui-image-credit">{c.attribution}</span>);
  // With a description the card reads as picture on the left, explanation on the right.
  const side = Boolean(c.description);

  if (image.status === "error") {
    const failed = <>
      <span className="chip danger">Image unavailable</span>
      <span className="md-image-error-msg">{image.error}</span>
      <button type="button" className="md-image-retry" onClick={e => { e.stopPropagation(); image.retry(); }}>Retry image</button>
    </>;
    if (!side) return <div className="gui-image-card is-error" role="group" aria-label={c.alt || c.title || "Image"}>{failed}{credit}</div>;
    return <div className="gui-image-card gui-image-side" role="group" aria-label={c.alt || c.title || "Image"}>
      <div className="gui-image-media is-error">{failed}</div>
      <div className="gui-image-body">
        {c.title && <h4 className="gui-image-title">{c.title}</h4>}
        <p className="gui-image-desc">{c.description}</p>
        {credit}
      </div>
    </div>;
  }

  const loading = <div className="gui-image-loading" role="status">
    <span className="pulse-dot" /> Loading {c.alt || "image"}…
  </div>;
  if (!image.displayUrl && !side) return loading;

  const picture = image.displayUrl ? <>
    <img className="md-inline-img gui-image-img" src={image.displayUrl} alt={c.alt || ""}
      title={c.title || c.alt || ""} onLoad={image.onLoad} onError={image.onError} />
    {image.displayKind === "thumbnail" && (
      <span className={`md-image-status${image.status === "full-error" ? " is-error" : ""}`} role="status">
        <span className="pulse-dot" />
        {image.status === "full-error" ? "Full image unavailable" : image.status === "preview" ? "Preview" : "Loading full image…"}
      </span>
    )}
    {image.status === "full-error" && (
      <button type="button" className="md-image-retry" onClick={e => { e.stopPropagation(); image.retry(); }}>Retry full image</button>
    )}
  </> : loading;

  if (side) {
    return <div className="gui-image-card gui-image-side">
      <div className="gui-image-media">{picture}</div>
      <div className="gui-image-body">
        {c.title && <h4 className="gui-image-title">{c.title}</h4>}
        <p className="gui-image-desc">{c.description}</p>
        {credit}
      </div>
    </div>;
  }
  return <div className="gui-image-card">
    {picture}
    {c.title && <span className="md-image-caption">{c.title}</span>}
    {credit}
  </div>;
}

// ============================================================================
// Component Registry for Display & Interactive Components
// ============================================================================

const GenerativeUiRegistry = {
  // ── Display Components ──
  text({ component: c }) {
    return <div className="gui-text" key={c.id}>
      {c.label && <strong className="gui-text-label">{c.label}</strong>}
      <p>{c.text}</p>
    </div>;
  },

  card({ component: c }) {
    return <div className="gui-item-card" key={c.id}>
      <div className="gui-item-card-head">
        {c.title && <h4 className="gui-item-card-title">{c.title}</h4>}
        {c.tag && <span className="gui-badge default">{c.tag}</span>}
      </div>
      {c.subtitle && <p className="gui-item-card-sub">{c.subtitle}</p>}
      {c.content && <p className="gui-item-card-body">{c.content}</p>}
    </div>;
  },

  stat({ component: c }) {
    const trendClass = c.trend === "up" ? "trend-up" : c.trend === "down" ? "trend-down" : "";
    return <div className="gui-stat" key={c.id}>
      <span className="gui-stat-label">{c.label}</span>
      <div className="gui-stat-value-row">
        <span className="gui-stat-val">{c.value}</span>
        {c.unit && <span className="gui-stat-unit">{c.unit}</span>}
      </div>
      {c.diff && <span className={`gui-stat-diff ${trendClass}`}>{c.diff}</span>}
    </div>;
  },

  badge({ component: c }) {
    const variant = c.variant || "default";
    return <div className="gui-badge-wrap" key={c.id}>
      <span className={`gui-badge ${variant}`}>{c.text}</span>
    </div>;
  },

  alert({ component: c }) {
    const level = c.level || "info";
    return <div className={`gui-alert ${level}`} role="alert" key={c.id}>
      {c.title && <strong className="gui-alert-title">{c.title}</strong>}
      <p className="gui-alert-msg">{c.message}</p>
    </div>;
  },

  progress({ component: c }) {
    const pct = Math.min(100, Math.max(0, (c.value / (c.max || 100)) * 100));
    return <div className="gui-progress" key={c.id}>
      <div className="gui-progress-head">
        {c.label && <span className="gui-progress-label">{c.label}</span>}
        <span className="gui-progress-val">{c.value} / {c.max}{c.unit ? ` ${c.unit}` : ""} ({Math.round(pct)}%)</span>
      </div>
      <div className="gui-progress-bar" role="progressbar" aria-valuenow={c.value} aria-valuemin={0} aria-valuemax={c.max}>
        <div className="gui-progress-fill" style={{ width: `${pct}%` }} />
      </div>
    </div>;
  },

  table({ component: c }) {
    return <div className="gui-table-wrap" key={c.id}>
      {c.title && <h4 className="gui-table-title">{c.title}</h4>}
      <table className="gui-table">
        <thead>
          <tr>
            {c.columns.map(col => <th key={col.key} style={{ textAlign: col.align || "left" }}>{col.label}</th>)}
          </tr>
        </thead>
        <tbody>
          {c.rows.map((row, i) => (
            <tr key={i}>
              {c.columns.map(col => (
                <td key={col.key} style={{ textAlign: col.align || "left" }}>{String(row[col.key] ?? "")}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>;
  },

  "key-value"({ component: c }) {
    return <div className="gui-key-value" key={c.id}>
      {c.title && <h4 className="gui-kv-title">{c.title}</h4>}
      <dl className="gui-kv-list">
        {c.items.map((item, i) => (
          <div key={item.key || i} className="gui-kv-row">
            <dt className="gui-kv-label">{item.label}</dt>
            <dd className="gui-kv-value">{item.value}</dd>
          </div>
        ))}
      </dl>
    </div>;
  },

  collapsible({ component: c }) {
    const [open, setOpen] = React.useState(() => c.defaultExpanded ?? false);
    return <details className="gui-collapsible" open={open} onToggle={e => setOpen(e.target.open)} key={c.id}>
      <summary className="gui-collapsible-sum">{c.title}</summary>
      <div className="gui-collapsible-body"><p>{c.content}</p></div>
    </details>;
  },

  tabs({ component: c }) {
    const [activeTab, setActiveTab] = React.useState(() => c.defaultTab ?? c.items[0]?.id);
    const activeItem = c.items.find(item => item.id === activeTab) || c.items[0];
    return <div className="gui-tabs" key={c.id}>
      <div className="gui-tabs-bar" role="tablist">
        {c.items.map(item => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={item.id === activeTab}
            className={`gui-tab-btn ${item.id === activeTab ? "is-active" : ""}`}
            onClick={() => setActiveTab(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div className="gui-tab-panel" role="tabpanel">
        <p>{activeItem?.content}</p>
      </div>
    </div>;
  },

  timeline({ component: c }) {
    return <div className="gui-timeline" key={c.id}>
      {c.title && <h4 className="gui-timeline-title">{c.title}</h4>}
      <ol className="gui-timeline-list">
        {c.items.map((item, i) => (
          <li key={i} className={`gui-timeline-item ${item.status || "pending"}`}>
            <div className="gui-timeline-marker" />
            <div className="gui-timeline-content">
              <span className="gui-timeline-time">{item.time}</span>
              <strong className="gui-timeline-heading">{item.title}</strong>
              {item.description && <p className="gui-timeline-desc">{item.description}</p>}
            </div>
          </li>
        ))}
      </ol>
    </div>;
  },

  "bar-chart"({ component: c }) {
    const Chart = window.ChartView;
    const spec = JSON.stringify({ type: "bar", title: c.title, unit: c.unit, data: c.data });
    if (Chart) return <div className="gui-chart-wrap" key={c.id}><Chart specText={spec} isClosed={true} /></div>;
    const maxVal = Math.max(...c.data.map(d => Math.abs(d.value)), 1);
    return <div className="gui-chart-fallback" key={c.id}>
      {c.title && <h4 className="gui-chart-title">{c.title}</h4>}
      <div className="gui-bar-list">
        {c.data.map((d, i) => (
          <div key={i} className="gui-bar-row">
            <span className="gui-bar-label">{d.label}</span>
            <div className="gui-bar-track">
              <div className="gui-bar-seg" style={{ width: `${Math.max(2, (d.value / maxVal) * 100)}%`, background: d.color || "var(--accent)" }} />
            </div>
            <span className="gui-bar-num">{d.value}{c.unit ? ` ${c.unit}` : ""}</span>
          </div>
        ))}
      </div>
    </div>;
  },

  "line-chart"({ component: c }) {
    const Chart = window.ChartView;
    const spec = JSON.stringify({ type: "line", title: c.title, unit: c.unit, data: c.data });
    if (Chart) return <div className="gui-chart-wrap" key={c.id}><Chart specText={spec} isClosed={true} /></div>;
    return GenerativeUiRegistry["bar-chart"]({ component: c });
  },

  "pie-chart"({ component: c }) {
    const Chart = window.ChartView;
    const spec = JSON.stringify({ type: "pie", title: c.title, unit: c.unit, data: c.data });
    if (Chart) return <div className="gui-chart-wrap" key={c.id}><Chart specText={spec} isClosed={true} /></div>;
    const total = c.data.reduce((sum, d) => sum + d.value, 0) || 1;
    return <div className="gui-chart-fallback" key={c.id}>
      {c.title && <h4 className="gui-chart-title">{c.title}</h4>}
      <div className="gui-pie-pbar">
        {c.data.map((d, i) => {
          const pct = (d.value / total) * 100;
          return <div key={i} className="gui-pie-seg" style={{ width: `${pct}%`, background: d.color || `hsl(${i * 60}, 65%, 55%)` }} title={`${d.label}: ${d.value}`} />;
        })}
      </div>
    </div>;
  },

  divider({ component: c }) {
    return <div className="gui-divider" key={c.id}>
      {c.label && <span className="gui-divider-label">{c.label}</span>}
    </div>;
  },

  image({ component: c }) {
    return <GenerativeUiImage key={c.id} component={c} />;
  },

  // ── Interactive Controls ──
  "option-cards"({ component: c, value, update, terminal }) {
    return <div className="gui-option-cards" role="radiogroup" aria-label={c.label}>
      {c.cards.map(card => {
        const isSelected = value === card.id;
        return <div
          key={card.id}
          role="radio"
          aria-checked={isSelected}
          tabIndex={terminal ? -1 : 0}
          className={`gui-option-card ${isSelected ? "is-selected" : ""} ${card.recommended ? "is-recommended" : ""}`}
          onClick={() => !terminal && update(c.id, card.id)}
          onKeyDown={e => { if (!terminal && (e.key === " " || e.key === "Enter")) { e.preventDefault(); update(c.id, card.id); } }}
        >
          <div className="gui-option-card-head">
            <strong className="gui-option-card-title">{card.title}</strong>
            <div className="gui-option-card-tags">
              {card.recommended && <span className="gui-badge success">Recommended</span>}
              {card.tag && <span className="gui-badge default">{card.tag}</span>}
            </div>
          </div>
          {card.description && <p className="gui-option-card-desc">{card.description}</p>}
        </div>;
      })}
    </div>;
  },
};

function GenerativeUiForm({ msg, onSubmit, onCancel, onRendered, disabled }) {
  const helper = window.OMP_GENERATIVE_UI;
  if (msg.error) throw new Error(msg.error);
  const schema = React.useMemo(() => helper.normalizeSchema(msg.schema), [msg.schema]);
  const isDisplayMode = schema.mode === "display";
  const identity = generativeUiIdentity(msg);
  const record = React.useRef(null);
  if (record.current === null) {
    record.current = generativeUiDrafts.get(identity) ?? { draft: helper.initialValues(schema), accepted: null, rendered: isDisplayMode };
    generativeUiRemember(identity, record.current);
  }
  const [draft, setDraft] = React.useState(() => helper.cloneValues(record.current.draft));
  const [accepted, setAccepted] = React.useState(record.current.accepted);
  const [pending, setPending] = React.useState(null);
  const pendingRef = React.useRef(false);
  const [errors, setErrors] = React.useState({});
  const [error, setError] = React.useState(null);
  const status = isDisplayMode ? "rendered" : (helper.isTerminal(msg.status) ? msg.status : accepted?.status ?? msg.status);
  const terminal = helper.isTerminal(status);
  const values = status === "submitted" && msg.values ? msg.values : accepted?.values ?? draft;
  const continuation = msg.continuation === "admitted" ? "admitted" : accepted?.continuation ?? msg.continuation;
  const resumable = !isDisplayMode && status === "submitted" && continuation === "pending";
  const Seg = window.SettingsSeg, Slider = window.SettingsSlider, Button = window.SettingsButton, Toggle = window.SettingsToggle;
  const locked = terminal || !!pending || disabled;
  const renderAttempted = React.useRef(false);

  React.useEffect(() => {
    if (disabled || terminal || !onRendered || renderAttempted.current || record.current.rendered || msg.rendered) return;
    renderAttempted.current = true;
    record.current.rendered = true;
    (async () => onRendered(msg))().catch(e => {
      record.current.rendered = false;
      setError(e.message);
    });
  }, [identity, disabled, terminal, msg.rendered, onRendered]);

  const update = (id, value) => {
    setDraft(old => {
      const next = { ...old, [id]: value };
      record.current.draft = helper.cloneValues(next);
      generativeUiRemember(identity, record.current);
      return next;
    });
    setErrors(old => { const next = { ...old }; delete next[id]; return next; });
    setError(null);
  };

  const act = async action => {
    if (pendingRef.current || (terminal && !(action === "resume" && resumable)) || disabled || isDisplayMode) return;
    if (action === "submit") {
      const invalid = helper.validateValues(schema, draft);
      setErrors(invalid);
      if (Object.keys(invalid).length) return;
    }
    pendingRef.current = true;
    setPending(action);
    setError(null);
    try {
      const submittedValues = helper.cloneValues(action === "resume" ? values : draft);
      const result = action === "cancel" ? await onCancel(msg) : await onSubmit(msg, submittedValues);
      const expected = action === "cancel" ? "cancelled" : "submitted";
      if (result?.status !== expected || result.ui_id !== msg.ui_id) throw new Error("The form action was not confirmed.");
      const receipt = { status: expected, continuation: result.continuation, ...(action !== "cancel" ? { values: submittedValues } : {}) };
      record.current.accepted = receipt;
      setAccepted(receipt);
    } catch (e) {
      setError(e.message);
    } finally {
      pendingRef.current = false;
      setPending(null);
    }
  };

  const displayValue = (c, value) => {
    if (c.type === "checkbox" || c.type === "switch") return value ? "On" : "Off";
    if (c.type === "multiselect" || c.type === "checkbox-group") return value?.length ? value.map(v => c.options.find(o => o.value === v)?.label ?? v).join(", ") : "None";
    if (c.type === "select" || c.type === "radio" || c.type === "segmented-control" || c.type === "searchable-select") return c.options.find(o => o.value === value)?.label ?? (value || "None");
    if (c.type === "option-cards") return c.cards.find(card => card.id === value)?.title ?? (value || "None");
    return value === "" ? "—" : value;
  };

  const eyebrowLabel = isDisplayMode ? "Display UI" : "Interactive form";
  const stateLabel = isDisplayMode
    ? "Displayed"
    : (status === "submitted" ? "Submitted" : status === "cancelled" ? "Cancelled" : status === "expired" ? "Expired" : "Waiting for input");

  return <section className={`gui-card gui-${schema.layout} ${isDisplayMode ? "gui-mode-display" : "gui-mode-interactive"}`} aria-label={schema.title ?? eyebrowLabel}>
    <header className="gui-head">
      <div>
        <span className="gui-eyebrow">{eyebrowLabel}</span>
        <h3>{schema.title ?? (isDisplayMode ? "Information overview" : "Choose your values")}</h3>
      </div>
      <span className={`gui-state${terminal ? " is-terminal" : ""}${status === "submitted" ? " is-submitted" : ""}`}>{stateLabel}</span>
    </header>
    <div className="gui-id">UI · {msg.ui_id}</div>

    {isDisplayMode ? (
      <div className="gui-fields">
        {schema.components.map(c => {
          const renderer = GenerativeUiRegistry[c.type];
          if (renderer) return renderer({ component: c });
          return null;
        })}
      </div>
    ) : (
      <form onSubmit={e => { e.preventDefault(); act("submit"); }} noValidate>
        <fieldset disabled={locked} className="gui-fields">
          {schema.components.filter(c => c.type !== "button" && helper.isComponentVisible(c, values)).map(c => {
            const value = values[c.id];
            const id = `gui-${msg.ui_id}-${c.id}`, errorId = `${id}-error`, labelId = `${id}-label`;
            const a11y = { "aria-invalid": !!errors[c.id], "aria-describedby": errors[c.id] ? errorId : undefined };

            // Check display component renderer inside interactive form
            if (GenerativeUiRegistry[c.type] && !helper.isInteractive(c)) {
              return GenerativeUiRegistry[c.type]({ component: c });
            }

            if (c.type === "option-cards") {
              return <div className="gui-field" key={c.id}>
                <label className="gui-label" id={labelId}>{c.label}{c.required && <span aria-hidden="true"> *</span>}</label>
                {terminal ? <output className="gui-result" aria-labelledby={labelId}>{displayValue(c, value)}</output> : GenerativeUiRegistry["option-cards"]({ component: c, value, update, terminal })}
                {errors[c.id] && <span className="gui-error" id={errorId} role="alert">{errors[c.id]}</span>}
              </div>;
            }

            return <div className="gui-field" key={c.id}>
              <label className="gui-label" id={labelId} htmlFor={id}>{c.label}{(c.required || c.minItems > 0) && <span aria-hidden="true"> *</span>}</label>
              {terminal ? <output className="gui-result" aria-labelledby={labelId}>{displayValue(c, value)}</output> : <>
                {c.type === "input" && <input id={id} type="text" value={value} placeholder={c.placeholder} required={c.required} minLength={c.minLength} maxLength={c.maxLength} {...a11y} onChange={e => update(c.id, e.target.value)} />}
                {c.type === "textarea" && <textarea id={id} value={value} placeholder={c.placeholder} rows={c.rows} required={c.required} minLength={c.minLength} maxLength={c.maxLength} {...a11y} onChange={e => update(c.id, e.target.value)} />}
                {(c.type === "date" || c.type === "date-picker") && <input id={id} type="date" value={value} min={c.min} max={c.max} required={c.required} {...a11y} onChange={e => update(c.id, e.target.value)} />}
                {c.type === "time-picker" && <input id={id} type="time" value={value} required={c.required} {...a11y} onChange={e => update(c.id, e.target.value)} />}
                {(c.type === "number" || c.type === "number-input") && <div className="gui-number-wrap"><input id={id} type="number" value={Number.isNaN(value) ? "" : value} min={c.min} max={c.max} step={c.step} {...a11y} onChange={e => update(c.id, e.target.valueAsNumber)} />{c.unit && <span className="gui-input-unit">{c.unit}</span>}</div>}
                {c.type === "checkbox" && <input id={id} type="checkbox" checked={value} {...a11y} onChange={e => update(c.id, e.target.checked)} />}
                {c.type === "switch" && <Toggle label={c.label} value={value} onChange={v => update(c.id, v)} />}
                {(c.type === "multiselect" || c.type === "checkbox-group") && <div className="gui-options" role="group" aria-labelledby={labelId} {...a11y}>{c.options.map((o, i) => <label key={o.value} htmlFor={`${id}-${i}`}><input id={`${id}-${i}`} type="checkbox" checked={value.includes(o.value)} onChange={e => update(c.id, e.target.checked ? [...value, o.value] : value.filter(v => v !== o.value))} /><span>{o.label}</span>{o.description && <small className="gui-opt-desc">{o.description}</small>}</label>)}</div>}
                {(c.type === "select" || c.type === "searchable-select") && <select id={id} value={value} required={c.required} {...a11y} onChange={e => update(c.id, e.target.value)}>{!c.required && <option value="">None</option>}{c.options.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}</select>}
                {(c.type === "radio" || c.type === "segmented-control") && <Seg label={c.label} value={value} options={!c.required ? [{ label: "None", value: "" }, ...c.options] : c.options} onChange={v => update(c.id, v)} />}
                {c.type === "slider" && <Slider label={c.label} value={value} min={c.min} max={c.max} step={c.step} onChange={v => update(c.id, v)} />}
              </>}
              {errors[c.id] && <span className="gui-error" id={errorId} role="alert">{errors[c.id]}</span>}
            </div>;
          })}
        </fieldset>
        {error && <p className="gui-error" role="alert">{error}</p>}
        <footer className="gui-footer">
          <span role="status">{status === "submitted" ? resumable ? "Values recorded. Resume to continue the task without submitting new values." : "Values recorded in the conversation." : terminal ? msg.reason ?? `This form is ${status}.` : pending ? (pending === "submit" ? "Submitting…" : "Cancelling…") : disabled ? "This form is unavailable in the current session." : "Changes stay here until you submit."}</span>
          {!terminal && <div className="gui-actions">{onCancel && <Button disabled={locked} onClick={() => act("cancel")}>Cancel</Button>}<Button accent disabled={locked} onClick={() => act("submit")}>{pending === "submit" ? "Submitting…" : schema.components.find(c => c.type === "button")?.label ?? "Submit"}</Button></div>}
          {resumable && <Button accent disabled={!!pending || disabled} onClick={() => act("resume")}>{pending === "resume" ? "Resuming…" : "Resume task"}</Button>}
        </footer>
      </form>
    )}
  </section>;
}

function GenerativeUiCard(props) {
  const identity = generativeUiIdentity(props.msg);
  return <GenerativeUiBoundary key={identity}><GenerativeUiForm key={identity} {...props} /></GenerativeUiBoundary>;
}

window.GenerativeUiCard = GenerativeUiCard;
