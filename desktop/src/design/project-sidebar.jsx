/* ═════════════════════════════════════════════════════════════════════
   project-sidebar.jsx — left project navigator (issue #27)

   `open`: every tab, grouped by project (folder + profile) exactly like the
   tab bar (`OMP_PROJECT_NAV.groupTabs`), multi-tab projects expandable.
   `recent`: recently opened folders of the active profile that have no
   open tab — computed by the caller (`recentRows`), since only it knows
   the active profile. Rows are containers; the click targets inside them
   are real buttons, so every action is keyboard-reachable.
   ═════════════════════════════════════════════════════════════════════ */

const { Icon, TabRunDot, TabLabel, RenameField } = window;
const { groupTabs, groupRunState, groupTarget, membersNewestFirst, basename, parentName } = window.OMP_PROJECT_NAV;

// Same guard as chrome.jsx's copy: the registry may not be loaded.
function sidebarHint(actionId, fallback) {
  return window.OMP_KEYMAP ? window.OMP_KEYMAP.hintFor(actionId) : fallback;
}

function ProjectSidebar({
  tabs, activeId, recents, profileLabel,
  onSelectTab, onCloseTab, onRenameTab, onNewInProject, onOpenRecent, onForgetRecent, onOpenFolder, onHide, onReorder, onHistory, footer, unread, onReveal, onMarkUnread, onArchiveTab, onDeleteTab,
}) {
  const ctx = window.useContextMenu();
  const tabMenu = t => [
    ...(t.path ? [{ label: "Open in Finder", onClick: () => onReveal(t.path) }, "-"] : []),
    { label: "Mark as unread", key: "U", onClick: () => onMarkUnread(t.id) },
    { label: "Rename", key: "R", onClick: () => startRename(t.id) },
    "-",
    { label: "Archive", key: "A", onClick: () => onArchiveTab(t) },
    { label: "Delete", key: "D", danger: true, confirm: "Delete permanently?", onClick: () => onDeleteTab(t) },
  ];
  // Group keys the user folded. Not persisted: tab ids (and so pathless
  // groups' keys) don't survive a restart anyway.
  const [collapsed, setCollapsed] = React.useState({});
  const toggle = key => setCollapsed(c => ({ ...c, [key]: !c[key] }));
  // Tab whose name is being edited inline (#32). Starting selects it, the
  // same as the first click of a double-click does.
  const [renaming, setRenaming] = React.useState(null);
  const startRename = id => { onSelectTab(id); setRenaming(id); };
  const renameField = t => (
    <RenameField value={t.name}
      onCommit={name => onRenameTab(t.id, name)}
      onClose={() => setRenaming(null)} />
  );
  const renameButton = t => (
    <button className="psb-act" title="Rename conversation" onClick={() => startRename(t.id)}>
      <Icon name="edit" size={10} />
    </button>
  );
  const groups = groupTabs(tabs);
  // Drag to reorder (#40), the same lists as the tab bar's: projects (a
  // project's rows move as one block), and members within their project.
  const reorder = window.useDragReorder(onReorder);

  return (
    <aside className="project-sidebar" aria-label="projects">
      <nav className="psb-nav">
        <button className="psb-navitem" onClick={onOpenFolder}>
          <Icon name="plus" size={15} /><span>New</span>
        </button>
        <button className="psb-navitem" onClick={onHistory}>
          <Icon name="clock" size={15} /><span>History</span>
        </button>
      </nav>

      <div className="psb-scroll" onPointerDown={reorder.onPointerDown}>
        <div className="psb-section">Open</div>
        {groups.length === 0 && <div className="psb-empty">no open tabs</div>}
        {groups.map(group => {
          const multi = group.tabs.length > 1;
          const expanded = multi && !collapsed[group.key];
          const containsActive = group.tabs.some(t => t.id === activeId);
          const profile = profileLabel(group.profile);
          const editing = !multi && renaming === group.tabs[0].id;
          const folderIcon = <Icon name="folder" size={11} color={containsActive ? "var(--accent)" : "var(--fg-4)"} />;
          // Single-tab cards show `folder · title` (#46), which ellipsizes.
          // The tooltip keeps the full title plus the folder path; multi-tab
          // project rows still tip the path (member rows tip each tab name).
          const cardTip = multi
            ? (group.path || group.name)
            : [group.tabs[0].name, group.path].filter(Boolean).join("\n");
          const members = `members:${group.key}`;
          return (
            <div key={group.key} {...reorder.itemProps("projects", group.key, "y")}
              className={`psb-group${reorder.dropClass("projects", group.key)}`}>
              {/* A multi-tab project row is only highlighted through its
                  active member row, as in the mockup; a single-tab one is
                  that tab. */}
              <div className={`psb-row psb-project ${containsActive && !expanded ? "active" : ""}`}
                onContextMenu={e => ctx.open(e, multi ? (group.path ? [{ label: "Open in Finder", onClick: () => onReveal(group.path) }] : []) : tabMenu(group.tabs[0]))}>
                {multi ? (
                  <button className={`psb-chev ${expanded ? "open" : ""}`}
                    title={expanded ? "collapse" : "expand"} aria-expanded={expanded}
                    onClick={e => { e.stopPropagation(); toggle(group.key); }}>
                    <Icon name="chevR" size={10} />
                  </button>
                ) : <span className="psb-chev-spacer" />}
                {editing ? (
                  <div className="psb-main psb-editing">
                    {folderIcon}
                    {renameField(group.tabs[0])}
                  </div>
                ) : (
                  <button className="psb-main" title={cardTip}
                    onClick={() => onSelectTab(groupTarget(group, activeId))}
                    onDoubleClick={() => { if (!multi) startRename(group.tabs[0].id); }}>
                    {folderIcon}
                    {multi
                      ? <span className="psb-name">{group.name}</span>
                      : <TabLabel tab={group.tabs[0]} className="psb-name" />}
                    {profile && (
                      <span className="chip muted tab-profile" title={`profile: ${profile}`}>{profile}</span>
                    )}
                    {group.tabs.some(x => unread?.has(x.id)) && <span className="psb-unread" title="Unread" />}
                    <TabRunDot state={groupRunState(group.tabs)} />
                    {multi && <span className="tab-count psb-count">{group.tabs.length}</span>}
                  </button>
                )}
                <span className="psb-actions">
                  {group.path && (
                    <button className="psb-act" title="New conversation here"
                      onClick={() => onNewInProject(group.path, group.profile)}>
                      <Icon name="plus" size={10} />
                    </button>
                  )}
                  {!multi && !editing && renameButton(group.tabs[0])}
                  {!multi && (
                    <button className="psb-act" title="Close tab" onClick={() => onCloseTab(group.tabs[0].id)}>
                      <Icon name="close" size={9} />
                    </button>
                  )}
                </span>
              </div>
              {expanded && membersNewestFirst(group).map(t => (
                <div key={t.id} {...reorder.itemProps(members, t.id, "y")}
                  className={`psb-row psb-tab ${t.id === activeId ? "active" : ""}${reorder.dropClass(members, t.id)}`}
                  onContextMenu={e => ctx.open(e, tabMenu(t))}>
                  {renaming === t.id ? (
                    <div className="psb-main psb-editing">{renameField(t)}</div>
                  ) : (
                    <button className="psb-main" title={t.name} onClick={() => onSelectTab(t.id)}
                      onDoubleClick={() => startRename(t.id)}>
                      <span className="psb-name">{t.name}</span>
                      <TabRunDot state={t.runState} />
                    </button>
                  )}
                  <span className="psb-actions">
                    {renaming !== t.id && renameButton(t)}
                    <button className="psb-act" title="Close tab" onClick={() => onCloseTab(t.id)}>
                      <Icon name="close" size={9} />
                    </button>
                  </span>
                </div>
              ))}
            </div>
          );
        })}

        <div className="psb-section">Recent</div>
        {recents.length === 0 && <div className="psb-empty">no recent projects</div>}
        {recents.map(r => (
          <div key={r.path} className="psb-row psb-recent">
            <span className="psb-chev-spacer" />
            <button className="psb-main" title={r.path} onClick={() => onOpenRecent(r.path)}>
              <Icon name="folder" size={11} color="var(--fg-5)" />
              <span className="psb-name">
                {basename(r.path)}
                <span className="psb-parent">{parentName(r.path)}</span>
              </span>
            </button>
            <span className="psb-actions">
              <button className="psb-act" title="Remove from recent" onClick={() => onForgetRecent(r.path)}>
                <Icon name="close" size={9} />
              </button>
            </span>
          </div>
        ))}
      </div>

      {footer}
      {ctx.menu && <window.ContextMenu menu={ctx.menu} onClose={ctx.close} />}
    </aside>
  );
}

Object.assign(window, { ProjectSidebar });
