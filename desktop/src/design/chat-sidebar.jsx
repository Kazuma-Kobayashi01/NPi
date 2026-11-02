/* ═════════════════════════════════════════════════════════════════════
   chat-sidebar.jsx — the left list in Chat mode: new chat, then every chat
   by day (open ones first under today, saved ones from disk below). Chats
   are omp sessions that run in the Chat folder (chat_space.rs), so `tabs`
   are only the open tabs of that folder and `saved` its history. A saved
   chat that is open is not listed twice (matched by session file).
   ═════════════════════════════════════════════════════════════════════ */

const { Icon: _CS_Icon, TabRunDot: _CS_Dot } = window;

const _csDay = (d) => {
  const today = new Date(); today.setHours(0, 0, 0, 0);
  const t = new Date(d); t.setHours(0, 0, 0, 0);
  const diff = Math.round((today - t) / 86400000);
  if (diff <= 0) return "Today";
  if (diff === 1) return "Yesterday";
  return new Date(d).toLocaleDateString("en-US", { month: "short", day: "numeric", ...(diff > 300 ? { year: "numeric" } : {}) });
};

const _csFile = (r) => (r.tab ? r.tab.sessionFile : r.saved?.path) || null;

/** Day groups, newest first, pinned ones first as their own group:
 *  `[{ label, items: [{ key, title, tab?, saved?, file }] }]`. Archived saved chats are left out. */
function groupChats(tabs, saved, pinned = [], archived = []) {
  const openFiles = new Set(tabs.map(t => t.sessionFile).filter(Boolean));
  const collapsedSaved = window.OMP_PROJECT_NAV.collapseChatFamilies(saved, openFiles);

  const isArchived = s => archived.includes(s.path)
    || (s._familyMembers && s._familyMembers.some(f => archived.includes(f)));

  const isPinned = r => r.file && (
    pinned.includes(r.file)
    || (r.saved?._familyMembers && r.saved._familyMembers.some(f => pinned.includes(f)))
  );

  const rows = [
    ...[...tabs].reverse().map(t => ({ key: `tab:${t.id}`, title: t.name, tab: t, at: Date.now() })),
    ...collapsedSaved.filter(s => !isArchived(s))
      .map(s => ({
        key: `saved:${s.path}`,
        title: s.title || "Untitled",
        saved: s,
        at: s._familyMaxAt || Date.parse(s.updated_at || s.timestamp) || 0,
      }))
      .sort((a, b) => b.at - a.at),
  ].map(r => {
    const file = _csFile(r);
    const session = saved.find(s => s.path === file);
    const familyFiles = r.saved?._familyMembers ?? (session
      ? saved.filter(s => s.family_path === session.family_path).map(s => s.path)
      : file ? [file] : []);
    return { ...r, file, familyFiles };
  });

  const groups = [];
  const pins = rows.filter(isPinned);
  if (pins.length) groups.push({ label: "Pinned", items: pins });
  for (const r of rows.filter(r => !isPinned(r))) {
    const label = _csDay(r.at);
    let g = groups[groups.length - 1];
    if (!g || g.label !== label || g.label === "Pinned") groups.push(g = { label, items: [] });
    g.items.push(r);
  }
  return groups;
}

function ChatSidebar({ tabs, saved, activeId, pinned = [], archived = [], unread, onSelectTab, onCloseTab, onResume, onNew, onHide, footer, onRename, onPin, onArchive, onDelete, onMarkUnread }) {
  const [folded, setFolded] = React.useState({});
  const [renaming, setRenaming] = React.useState(null);
  const ctx = window.useContextMenu();
  const groups = React.useMemo(() => groupChats(tabs, saved, pinned, archived), [tabs, saved, pinned, archived]);
  const menuFor = (r) => {
    const isPinned = !!r.file && (
      pinned.includes(r.file)
      || (r.saved?._familyMembers && r.saved._familyMembers.some(f => pinned.includes(f)))
    );
    return [
      ...(r.file ? [{ label: isPinned ? "Unpin" : "Pin", key: "P", onClick: () => onPin(r.file) }] : []),
      ...(r.tab ? [{ label: "Mark as unread", key: "U", onClick: () => onMarkUnread(r.tab.id) }, { label: "Rename", key: "R", onClick: () => { onSelectTab(r.tab.id); setRenaming(r.tab.id); } }] : []),
      "-",
      { label: "Archive", key: "A", onClick: () => onArchive(r) },
      { label: "Delete conversation", key: "D", danger: true,
        confirm: `Delete conversation and all branches (${r.familyFiles.length} files)?`,
        onClick: () => onDelete(r) },
    ];
  };
  return (
    <aside className="project-sidebar chat-sidebar" aria-label="chats">
      <nav className="psb-nav">
        <button className="psb-navitem" onClick={onNew}>
          <_CS_Icon name="plus" size={15} /><span>New chat</span>
        </button>
      </nav>
      <div className="psb-scroll">
        {groups.length === 0 && <div className="psb-empty">No chats yet</div>}
        {groups.map(g => (
          <div key={g.label} className="chs-group">
            <button type="button" className="chs-day" aria-expanded={!folded[g.label]}
              onClick={() => setFolded(f => ({ ...f, [g.label]: !f[g.label] }))}>
              <span>{g.label}</span>
              <svg width="9" height="9" viewBox="0 0 10 10" className={folded[g.label] ? "" : "open"} aria-hidden="true">
                <path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            {!folded[g.label] && g.items.map(r => (
              <div key={r.key} className={`psb-row psb-tab ${r.tab && r.tab.id === activeId ? "active" : ""}`}
                onContextMenu={e => ctx.open(e, menuFor(r))}>
                {r.tab && renaming === r.tab.id ? (
                  <div className="psb-main psb-editing">
                    <window.RenameField value={r.tab.name} onCommit={name => onRename(r.tab.id, name)} onClose={() => setRenaming(null)} />
                  </div>
                ) : (
                  <button className="psb-main" title={r.title}
                    onClick={() => (r.tab ? onSelectTab(r.tab.id) : onResume(r.saved))}>
                    <span className="psb-name">{r.title}</span>
                    {r.tab && unread?.has(r.tab.id) && <span className="psb-unread" title="Unread" />}
                    {r.tab && <_CS_Dot state={r.tab.runState} />}
                  </button>
                )}
                {r.tab && (
                  <span className="psb-actions">
                    <button className="psb-act" title="Close chat" onClick={() => onCloseTab(r.tab.id)}>
                      <_CS_Icon name="close" size={9} />
                    </button>
                  </span>
                )}
              </div>
            ))}
          </div>
        ))}
      </div>
      {footer}
      {ctx.menu && <window.ContextMenu menu={ctx.menu} onClose={ctx.close} />}
    </aside>
  );
}

Object.assign(window, { ChatSidebar });
