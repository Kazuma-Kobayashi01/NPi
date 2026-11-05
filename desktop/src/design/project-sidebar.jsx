/* ═════════════════════════════════════════════════════════════════════
   project-sidebar.jsx — left project navigator (issue #27)

   Open tabs and saved main conversations live beneath their project, even
   when there is only one conversation. Closed projects retain resumable
   conversation rows under Recent. Rows are containers; the click targets
   inside them are real buttons, so every action is keyboard-reachable.
   ═════════════════════════════════════════════════════════════════════ */

// The UI language (app/i18n.js).
const _PSB_T = (key, vars) => window.OMP_I18N.t(key, vars);
const { Icon, TabRunDot, RenameField } = window;
const { groupTabs, groupRunState, groupTarget, membersNewestFirst, basename, parentName } = window.OMP_PROJECT_NAV;

// Same guard as chrome.jsx's copy: the registry may not be loaded.
function sidebarHint(actionId, fallback) {
  return window.OMP_KEYMAP ? window.OMP_KEYMAP.hintFor(actionId) : fallback;
}

function ProjectSidebar({
  tabs, activeId, recents, profileLabel, saved = [], historyProfile, onResume,
  onSelectTab, onCloseTab, onCloseProject, onRenameTab, onNewInProject, onNewWorktree, onOpenRecent, onForgetRecent, onOpenFolder, onHide, onReorder, onHistory, footer, unread, onReveal, onMarkUnread, onArchiveTab, onDeleteTab, mode, onMode,
}) {
  const ctx = window.useContextMenu();
  const tabMenu = t => [
    ...(t.path ? [{ label: _PSB_T("Open in Finder"), onClick: () => onReveal(t.path) }, "-"] : []),
    { label: _PSB_T("Mark as unread"), key: "U", onClick: () => onMarkUnread(t.id) },
    { label: _PSB_T("Rename"), key: "R", onClick: () => startRename(t.id) },
    "-",
    { label: _PSB_T("Close"), onClick: () => onCloseTab(t.id) },
    { label: _PSB_T("Archive"), key: "A", onClick: () => onArchiveTab(t) },
    { label: _PSB_T("Delete"), key: "D", danger: true, confirm: _PSB_T("Delete permanently?"), onClick: () => onDeleteTab(t) },
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
    <button className="psb-act" title={_PSB_T("Rename conversation")} onClick={() => startRename(t.id)}>
      <Icon name="edit" size={10} />
    </button>
  );
  const groups = groupTabs(tabs);
  const savedForProject = (path, profile) => {
    const projectSaved = saved.filter(s => s.profile === profile
      && window.OMP_PROJECT_NAV.normPath(s.cwd) === window.OMP_PROJECT_NAV.normPath(path));
    const openFiles = window.OMP_PROJECT_NAV.openFilesOf(tabs.filter(t => t.profile === profile));
    return window.OMP_PROJECT_NAV.collapseChatFamilies(projectSaved, openFiles);
  };
  const savedRow = s => (
    <div key={s.path} className="psb-row psb-tab">
      <button className="psb-main" title={s.title} onClick={() => onResume(s)}>
        <span className="psb-name">{s.title}</span>
      </button>
    </div>
  );
  // Drag to reorder (#40), the same lists as the tab bar's: projects (a
  // project's rows move as one block), and members within their project.
  const reorder = window.useDragReorder(onReorder);

  return (
    <aside className="project-sidebar" aria-label={_PSB_T("projects")}>
      <window.ModeSwitch mode={mode} onMode={onMode} />
      <nav className="psb-nav">
        <button className="psb-navitem" onClick={onOpenFolder}>
          <Icon name="plus" size={15} /><span>{_PSB_T("New")}</span>
        </button>
        <button className="psb-navitem" onClick={onHistory}>
          <Icon name="clock" size={15} /><span>{_PSB_T("History")}</span>
        </button>
      </nav>

      <div className="psb-scroll" onPointerDown={reorder.onPointerDown}>
        <div className="psb-section">{_PSB_T("Open")}</div>
        {groups.length === 0 && <div className="psb-empty">{_PSB_T("no open tabs")}</div>}
        {groups.map(group => {
          const history = savedForProject(group.path, group.profile);
          const expanded = !collapsed[group.key];
          const containsActive = group.tabs.some(t => t.id === activeId);
          const profile = profileLabel(group.profile);
          const folderIcon = <Icon name="folder" size={11} color={containsActive ? "var(--accent)" : "var(--fg-4)"} />;
          const cardTip = group.path || group.name;
          const members = `members:${group.key}`;
          return (
            <div key={group.key} {...reorder.itemProps("projects", group.key, "y")}
              className={`psb-group${reorder.dropClass("projects", group.key)}`}>
              <div className={`psb-row psb-project ${containsActive && !expanded ? "active" : ""}`}
                onContextMenu={e => ctx.open(e, [
                  ...(group.path ? [{ label: _PSB_T("Open in Finder"), onClick: () => onReveal(group.path) }, "-"] : []),
                  ...(group.path && onNewWorktree ? [{ label: _PSB_T("New task in a worktree"), onClick: () => onNewWorktree(group.path, group.profile) }, "-"] : []),
                  { label: _PSB_T("Close"), onClick: () => onCloseProject(group.tabs) },
                ])}>
                  <button className={`psb-chev ${expanded ? "open" : ""}`}
                    title={_PSB_T(expanded ? "collapse" : "expand")} aria-expanded={expanded}
                    onClick={e => { e.stopPropagation(); toggle(group.key); }}>
                    <Icon name="chevR" size={10} />
                  </button>
                  <button className="psb-main" title={cardTip}
                    onClick={() => onSelectTab(groupTarget(group, activeId))}>
                    {folderIcon}
                    <span className="psb-name">{group.name}</span>
                    {profile && (
                      <span className="chip muted tab-profile" title={_PSB_T("profile: {name}", { name: profile })}>{profile}</span>
                    )}
                    {group.tabs.some(x => unread?.has(x.id)) && <span className="psb-unread" title={_PSB_T("Unread")} />}
                    <TabRunDot state={groupRunState(group.tabs)} />
                    <span className="tab-count psb-count">{group.tabs.length + history.length}</span>
                  </button>
                <span className="psb-actions">
                  {group.path && onNewWorktree && (
                    <button className="psb-act" title={_PSB_T("New task in a worktree")}
                      onClick={() => onNewWorktree(group.path, group.profile)}>
                      <Icon name="branch" size={10} />
                    </button>
                  )}
                  {group.path && (
                    <button className="psb-act" title={_PSB_T("New conversation here")}
                      onClick={() => onNewInProject(group.path, group.profile)}>
                      <Icon name="plus" size={10} />
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
                      {t.worktree && <span title={t.worktree.branch}><Icon name="branch" size={9} color="var(--accent)" /></span>}
                      <span className="psb-name">{t.name}</span>
                      <TabRunDot state={t.runState} />
                    </button>
                  )}
                  <span className="psb-actions">
                    {renaming !== t.id && renameButton(t)}
                    <button className="psb-act" title={_PSB_T("Close tab")} onClick={() => onCloseTab(t.id)}>
                      <Icon name="close" size={9} />
                    </button>
                  </span>
                </div>
              ))}
              {expanded && history.map(savedRow)}
            </div>
          );
        })}

        <div className="psb-section">{_PSB_T("Recent")}</div>
        {recents.length === 0 && <div className="psb-empty">{_PSB_T("no recent projects")}</div>}
        {recents.map(r => {
          const key = window.OMP_PROJECT_NAV.groupKey({ path: r.path, profile: historyProfile });
          const expanded = !collapsed[key];
          const history = savedForProject(r.path, historyProfile);
          return (
            <div key={key} className="psb-group">
              <div className="psb-row psb-recent">
                <button className={`psb-chev ${expanded ? "open" : ""}`}
                  title={_PSB_T(expanded ? "collapse" : "expand")} aria-expanded={expanded}
                  onClick={() => toggle(key)}>
                  <Icon name="chevR" size={10} />
                </button>
                <button className="psb-main" title={r.path} onClick={() => toggle(key)}>
                  <Icon name="folder" size={11} color="var(--fg-5)" />
                  <span className="psb-name">
                    {basename(r.path)}
                    <span className="psb-parent">{parentName(r.path)}</span>
                  </span>
                  <span className="tab-count psb-count">{history.length}</span>
                </button>
                <span className="psb-actions">
                  <button className="psb-act" title={_PSB_T("New conversation here")} onClick={() => onOpenRecent(r.path)}>
                    <Icon name="plus" size={10} />
                  </button>
                  <button className="psb-act" title={_PSB_T("Remove from recent")} onClick={() => onForgetRecent(r.path)}>
                    <Icon name="close" size={9} />
                  </button>
                </span>
              </div>
              {expanded && history.map(savedRow)}
              {expanded && history.length === 0 && <div className="psb-empty">{_PSB_T("No saved conversations")}</div>}
            </div>
          );
        })}
      </div>

      {footer}
      {ctx.menu && <window.ContextMenu menu={ctx.menu} onClose={ctx.close} />}
    </aside>
  );
}

Object.assign(window, { ProjectSidebar });
