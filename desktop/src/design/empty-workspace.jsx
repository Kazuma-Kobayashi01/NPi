/* ═════════════════════════════════════════════════════════════════════
   empty-workspace.jsx — the start page (no tab open, or the back button)

   A greeting, a prompt box that is one line until it is focused, and under
   it the choices a new conversation needs: the project or folder on the
   left; the model and effort, and the tool approval mode on the right. A
   "+" inside the box attaches images. Enter opens that folder as a tab,
   applies the choices to it and sends the message. Notes the bridge filed
   while no tab existed (e.g. a failed "Open with") show underneath.
   ═════════════════════════════════════════════════════════════════════ */

const { MarkdownContent, Icon, ModelMenu } = window;
const { basename, parentName } = window.OMP_PROJECT_NAV;

// The NPi portico, drawn with the text colour.
function WelcomeMark() {
  return (
    <svg className="welcome-mark" width="40" height="40" viewBox="100 108 312 272" fill="currentColor" aria-hidden="true">
      <rect x="116" y="116" width="280" height="14" />
      <rect x="136" y="136" width="240" height="34" />
      {[182, 330].map(c => (
        <g key={c}>
          <rect x={c - 24} y="169" width="48" height="11" />
          <polygon points={`${c - 20},179 ${c + 20},179 ${c + 13},192 ${c - 13},192`} />
          <polygon points={`${c - 13},191 ${c + 13},191 ${c + 17},343 ${c - 17},343`} />
          <rect x={c - 26} y="341" width="52" height="14" />
        </g>
      ))}
      <rect x="124" y="353" width="264" height="10" />
      <rect x="104" y="361" width="304" height="11" />
      <circle cx="256" cy="272" r="15" />
    </svg>
  );
}

const _ewCap = (t) => (t ? t.charAt(0).toUpperCase() + t.slice(1) : t);

function EmptyWorkspace({ notes, recents = [], onPickFolder, onStart, bridge, approvalModes, profileId, favorites = [], onManageModels, wallpaper = null, chat = "", appMode = "code", userName = "" }) {
  const M = window.OMP_MODELS;
  const IMG = window.OMP_IMAGES;
  const [text, setText] = React.useState("");
  const [focused, setFocused] = React.useState(false);
  const [picked, setPicked] = React.useState(null);
  const [menu, setMenu] = React.useState(false);
  const [modeMenu, setModeMenu] = React.useState(false);
  const [atts, setAtts] = React.useState([]);
  const [catalog, setCatalog] = React.useState([]);
  const [defaultSel, setDefaultSel] = React.useState("");
  const [modelKey, setModelKey] = React.useState(null);   // null: omp's default
  const [level, setLevel] = React.useState(null);         // null: the default's level
  // The greeting: drawn once per visit to the page (and again when the mode changes).
  const [seed, setSeed] = React.useState(() => Math.random());
  React.useEffect(() => { setSeed(Math.random()); }, [appMode]);
  const title = React.useMemo(
    () => window.OMP_GREETINGS.greeting({ mode: appMode, name: userName, hour: new Date().getHours(), seed }),
    [appMode, userName, seed],
  );
  const taRef = React.useRef(null);
  const rootRef = React.useRef(null);
  const fileRef = React.useRef(null);
  const counter = React.useRef(0);

  // Chat: the project is the Chat folder, so there is nothing to choose.
  const project = chat || (picked ?? recents[0]?.path ?? null);
  const open = focused || text !== "" || atts.length > 0;
  const canSend = !!project && (text.trim() !== "" || atts.length > 0);

  // The models and the default come from omp's CLI: no session is needed.
  React.useEffect(() => {
    let live = true;
    Promise.resolve(bridge?.modelsCatalog?.(profileId)).then(r => { if (live && r?.ok && Array.isArray(r.value)) setCatalog(r.value); });
    Promise.resolve(bridge?.defaultModel?.(profileId)).then(r => { if (live && r?.ok) setDefaultSel(r.value ?? ""); });
    return () => { live = false; };
  }, [bridge, profileId]);

  const [defKey, defLevel] = React.useMemo(() => {
    const [k, l] = String(defaultSel || "").split(":");
    return [k || null, l || ""];
  }, [defaultSel]);
  const current = React.useMemo(() => {
    const byKey = new Map(catalog.map(m => [M.keyOf(m), m]));
    return byKey.get(modelKey ?? defKey) ?? catalog[0] ?? null;
  }, [catalog, modelKey, defKey]);
  const shownLevel = (() => {
    const want = level ?? (modelKey ? "" : defLevel);
    const have = current?.thinking ?? [];
    return have.includes(want) ? want : (have.includes("medium") ? "medium" : (have[0] ?? ""));
  })();

  const mode = window.OMP_APPROVAL_MODE.effectiveFor(approvalModes, project);

  // Close the pickers on any press outside them.
  React.useEffect(() => {
    if (!menu && !modeMenu) return undefined;
    const off = e => {
      if (!rootRef.current?.querySelector(".welcome-picker")?.contains(e.target)) setMenu(false);
      if (!rootRef.current?.querySelector(".welcome-mode")?.contains(e.target)) setModeMenu(false);
    };
    document.addEventListener("mousedown", off);
    return () => document.removeEventListener("mousedown", off);
  }, [menu, modeMenu]);

  // Grow with the text, up to a cap.
  React.useLayoutEffect(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${Math.min(ta.scrollHeight, 240)}px`;
    ta.style.overflowY = ta.scrollHeight > 240 ? "auto" : "hidden";
  }, [text, open]);

  const addFiles = async (list) => {
    const files = Array.from(list || []).filter(f => f.type.startsWith("image/")).slice(0, IMG.MAX_ATTACHMENTS - atts.length);
    const made = await Promise.all(files.map(async f => {
      try { const image = await IMG.prepareImage(f); return { id: `w-${++counter.current}`, name: f.name, image, src: IMG.toDataUrl(image) }; }
      catch { return null; }
    }));
    setAtts(a => [...a, ...made.filter(Boolean)].slice(0, IMG.MAX_ATTACHMENTS));
  };

  const send = () => {
    if (!canSend) return;
    onStart?.(project, text.trim(), {
      model: modelKey ? current : null,
      level: level || (modelKey ? shownLevel : ""),
      images: atts.map(a => a.image),
    });
    setText("");
    setAtts([]);
  };
  const onKey = e => {
    // Enter during IME composition confirms the candidate, it does not send.
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing && e.keyCode !== 229) {
      e.preventDefault();
      send();
    }
  };
  const chooseOther = async () => {
    setMenu(false);
    const path = await onPickFolder?.();
    if (path) { setPicked(path); taRef.current?.focus(); }
  };
  const setMode = async (value) => {
    setModeMenu(false);
    if (project) await bridge?.setApprovalModeProject?.(project, value);
    else await bridge?.setApprovalModeDefault?.(value);
  };

  return (
    <div className="empty-ws" aria-label="no project open" ref={rootRef}>
      {wallpaper && <div className="wallpaper" style={{ "--wp": `url("${new URL(wallpaper, document.baseURI).href}")` }} aria-hidden="true" />}
      <div className="welcome">
        <h1 className="welcome-title"><WelcomeMark />{title}</h1>

        <div className={`welcome-box${open ? " open" : ""}`} onMouseDown={e => { if (e.target === e.currentTarget) taRef.current?.focus(); }}>
          {atts.length > 0 && (
            <div className="welcome-atts">
              {atts.map(a => (
                <span key={a.id} className="welcome-att">
                  <img src={a.src} alt={a.name || "attached image"} />
                  <button type="button" aria-label="Remove image" onClick={() => setAtts(l => l.filter(x => x.id !== a.id))}>×</button>
                </span>
              ))}
            </div>
          )}
          <div className="welcome-line">
            <button type="button" className="welcome-plus" title="Add images" aria-label="Add images"
              onMouseDown={e => e.preventDefault()} onClick={() => fileRef.current?.click()}>
              <Icon name="plus" size={16} />
            </button>
            <input ref={fileRef} type="file" accept="image/*" multiple style={{ display: "none" }}
              onChange={e => { addFiles(e.target.files); e.target.value = ""; }} />
            <textarea ref={taRef} className="welcome-ta selectable" rows="1" value={text}
              placeholder="How can I help you today?"
              onChange={e => setText(e.target.value)}
              onKeyDown={onKey}
              onPaste={e => {
                const files = Array.from(e.clipboardData?.files ?? []).filter(f => f.type.startsWith("image/"));
                if (files.length) { e.preventDefault(); addFiles(files); }
              }}
              onFocus={() => setFocused(true)}
              onBlur={() => setFocused(false)} />
            <button type="button" className="welcome-send" disabled={!canSend} onMouseDown={e => e.preventDefault()} onClick={send}
              title={project ? "Start" : "Choose a folder first"} aria-label="Start">
              <Icon name="arrow" size={13} />
            </button>
          </div>
        </div>

        <div className="welcome-row">
          {!chat && <div className="welcome-picker">
            <button type="button" className="welcome-pick" aria-haspopup="menu" aria-expanded={menu} onClick={() => setMenu(v => !v)}>
              <Icon name="folder" size={14} />
              <span>{project ? basename(project) : "Project or folder"}</span>
              <Icon name="chev" size={10} />
            </button>
            {menu && (
              <div className="welcome-menu" role="menu">
                {recents.map(r => (
                  <button key={r.path} type="button" role="menuitem" className={`welcome-item${r.path === project ? " on" : ""}`}
                    title={r.path} onClick={() => { setPicked(r.path); setMenu(false); taRef.current?.focus(); }}>
                    <span className="welcome-item-name">{basename(r.path)}</span>
                    <span className="welcome-item-dir">{parentName(r.path)}</span>
                  </button>
                ))}
                {recents.length > 0 && <div className="welcome-sep" />}
                <button type="button" role="menuitem" className="welcome-item" onClick={chooseOther}>
                  <Icon name="plus" size={13} /><span className="welcome-item-name">Open another folder…</span>
                </button>
              </div>
            )}
          </div>}

          <div className="welcome-choices">
            <ModelMenu up={false}
              models={catalog} current={current} favorites={favorites} level={shownLevel}
              loadLevels={() => Promise.resolve(current?.thinking ?? [])}
              loadRecent={() => bridge?.recentModels?.(profileId)}
              onPick={m => { setModelKey(M.keyOf(m)); setLevel(null); }}
              onSetLevel={setLevel}
              onManage={onManageModels} />
            <div className="welcome-mode">
              <button type="button" className={`mm-btn${modeMenu ? " on" : ""}`} aria-haspopup="menu" aria-expanded={modeMenu}
                title="Tool approvals" onClick={() => setModeMenu(v => !v)}>
                <span className={`mm-btn-name${mode === "yolo" ? " warn" : ""}`}>{_ewCap(window.OMP_APPROVAL_MODE.labelOf(mode))}</span>
              </button>
              {modeMenu && (
                <div className="mm-pop down" role="menu">
                  {window.OMP_APPROVAL_MODE.MODES.map(m => (
                    <button key={m.value} type="button" role="menuitemradio" aria-checked={m.value === mode}
                      className={`mm-row${m.value === mode ? " on" : ""}`} onClick={() => setMode(m.value)}>
                      <span className="mm-name">{_ewCap(m.label)}</span>
                      <span className="mm-right">{m.value === mode ? (
                        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m3.5 8.5 3 3 6-7" stroke="var(--accent)" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>
                      ) : null}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        {notes.length > 0 && (
          <div className="empty-ws-notes">
            {notes.map((note, i) => (
              <div key={i} className="empty-ws-note">
                <MarkdownContent text={note} />
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

Object.assign(window, { EmptyWorkspace });
