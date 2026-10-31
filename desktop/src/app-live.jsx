/* ═════════════════════════════════════════════════════════════════════
   app-live.jsx — live-wired root.

   Session model: each tab owns one omp process. OMP_BRIDGE manages
   session lifecycle; the tab list and active session come from the
   bridge (snap.sessions / snap.activeSessionId). Switching tabs calls
   bridge.activateSession() which resets ALL per-session state and
   re-fetches from omp — so the right panel (sparkline, activity radar,
   minimap, kanban, context gauge) always reflects the active session.

   Constants and the cross-cutting effects (bridge subscription, theme)
   live in app/constants.js and app/use-bridge-snapshot.jsx respectively.
   Keyboard shortcuts are resolved by app/use-keymap.jsx (useKeymap loads
   the backend payload and resolves it; useKeymapDispatch installs the
   single window keydown listener) and handled by the handler map built
   below. This file owns only the App component itself: state
   declarations, handlers, and the render tree.
   ═════════════════════════════════════════════════════════════════════ */

const {
  Icon, ChatView, Composer, CommandBridge, WindowChrome, SubagentPane, ProjectSidebar, EmptyWorkspace,
  ChromeToolbar, AmbientRail, ConversationTree, PlanKanban, HistoryModal, GitBrowser, FilePreview, ApprovalRulesPanel, UsageStatsPanel, PromptHistoryModal, useTweaks,
  SettingsPage, ConfirmCloseDialog,
  useWidgets, WidgetCard, WidgetErrorCard, WidgetApprovalCard, WidgetBoundary,
  TWEAK_DEFAULTS, NULL_MODEL, EMPTY_PROJECT, DEFAULT_PROFILE_ID,
  INTENT_FRAMING, APPROVAL_PROMPT,
  useBridgeSnapshot, useThemeEffect, useSubagentManager, useConversationTree, useGoalMode, useTerminal, TerminalPanels, timeNow,
  useKeymap, useKeymapDispatch, useBrowser, BrowserDock, MarkdownLinkContext: _AL_MdLinkCtx,
  usePaneWidths, ResizeHandle, useQuotas, usePrAutomation, PrAutomationCard,
} = window;
const { routeSubmit: _routeSubmit } = window.OMP_SLASH;
const { mergeIntoMessage: _mergeBrowserChips, browserCommand: _browserCommand, MIN_WIDTH: _AL_BROWSER_MIN } = window.OMP_BROWSER;
const { recentRows, groupKey, moveGroup, moveMember, shiftGroup } = window.OMP_PROJECT_NAV;
const {
  entryOf, updateEntry, pruneEntries, seedEntries,
  PLAN_IDLE, togglePlan, enterPlan, approvePlan, annotatePlan, markPlanSent,
} = window.OMP_SESSION_UI;
const { NO_OVERRIDES: NO_COLLAPSE_OVERRIDES, foldModes: _foldModes, overridesForAll: _collapseOverridesForAll } = window.OMP_COLLAPSE;
const { tweakKey: _widgetTweakKey } = window.OMP_WIDGETS;
const { EMPTY: _EXT_EMPTY, cards: _extCards, toggles: _extToggles } = window.OMP_EXT_WIDGETS;
const { entryOf: _TERM_entryOf } = window.OMP_TERMINAL;
const { absolutePath: _gitAbsolutePath, finishedEdits: _gitFinishedEdits } = window.OMP_GIT_BROWSER;
const { linkClick: _fpLinkClick } = window.OMP_FILE_PREVIEW;

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const data          = window.OMP_DATA;
  const bridge        = window.OMP_BRIDGE;

  // ── UI state ──────────────────────────────────────────────────────────────
  const [bridgeOpen,  setBridgeOpen]  = React.useState(false);
  const [bridgeView,  setBridgeView]  = React.useState("commands");
  const [historyOpen, setHistoryOpen] = React.useState(false);
  // Git browser: null, or { path, project } — the file it opens on (null =
  // the first changed file) and the project that path belongs to.
  const [gitOpen,     setGitOpen]     = React.useState(null);
  // The file preview (issue #39): `{ key, target }`, `key` the tab it was
  // opened in — it shows only while that tab is active.
  const [preview,     setPreview]     = React.useState(null);
  const [rulesOpen,   setRulesOpen]   = React.useState(false);
  const [statsOpen,   setStatsOpen]   = React.useState(false);
  const [promptHistoryOpen, setPromptHistoryOpen] = React.useState(false);
  // The Settings app tab (#37; app/settings-tab.js): `{open, active,
  // section}`. Not an omp tab: the active omp tab stays `activeSessionId`
  // underneath while the page covers the stage.
  const [stab, setStab] = React.useState(window.OMP_SETTINGS_TAB.initial);
  // The start page shown over open tabs (the top bar's back button); the
  // forward button returns to the conversation.
  const [home, setHome] = React.useState(false);
  // live.js's pending "close a working tab / quit?" question, or null.
  const [closeConfirm, setCloseConfirm] = React.useState(null);
  const [planOpen,    setPlanOpen]    = React.useState(false);
  // The column beside the chat, per tab (`side` below): the ambient rail,
  // the conversation tree (prompt-cache navigator; design/conversation-tree.jsx,
  // app/use-conversation-tree.jsx) or the browser panel — one at a time.
  const [sides, setSides] = React.useState({});
  // Plan mode is per tab (issue #28; app/session-ui.js): one entry per
  // session id, read through `plan` and written through `updatePlan` below.
  const [plans, setPlans] = React.useState({});
  // Collapsed/expanded tool cards and thinking the user toggled (#44;
  // app/chat-collapse.js): per tab, `_id → collapsed`, so a tab switch —
  // which remounts ChatView — keeps them.
  const [collapseOverrides, setCollapseOverrides] = React.useState({});

  // Cross-component highlight: hovering a minimap cell lights up the
  // matching chat bubble; clicking scrolls to it.
  const [hoveredMsgIdx, setHoveredMsgIdx] = React.useState(null);
  const handleMinimapClick = (idx) => {
    const el = document.querySelector(`[data-msg-idx="${idx}"]`);
    el?.scrollIntoView({ block: "center", behavior: "smooth" });
  };

  // ── Live data (all per-session — driven by OMP_BRIDGE.onUpdate) ───────────
  const [messages,      setMessages]      = React.useState([]);
  const [streaming,     setStreaming]     = React.useState(false);
  const [model,         setModelState]    = React.useState(NULL_MODEL);
  const [thinkingLevel, setThinkingLevel] = React.useState(null);
  const [ctx,           setCtx]           = React.useState(data.ctx);
  const [sessionStats,   setSessionStats]   = React.useState(null);
  const [ctxBreakdown,   setCtxBreakdown]   = React.useState(null);
  const [kanban,        setKanban]        = React.useState([]);
  const [planMeta,      setPlanMeta]      = React.useState(data.planMeta);
  const [models,        setModels]        = React.useState([]);
  const [activity,      setActivity]      = React.useState([]);
  const [sparkline,     setSparkline]     = React.useState(Array(30).fill(0));
  const [loginProviders, setLoginProviders] = React.useState(null);
  // Active tab's in-memory prompt history (issue #16) — see live.js
  // promptHistories. Never persisted; re-sourced from the bridge snapshot
  // on every update, same as any other per-session field here.
  const [promptHistory, setPromptHistory] = React.useState([]);
  // Subagent manager (src/app/subagents.js shape) + the active tab's
  // on-demand agent transcripts.
  const [subagents,           setSubagents]           = React.useState(() => window.OMP_SUBAGENTS.emptySubagents());
  const [subagentTranscripts, setSubagentTranscripts] = React.useState({});
  // omp's steer / follow-up queue + this tab's unacknowledged sends
  // (app/message-queue.js), rendered by the composer's queue strip.
  const [queue,        setQueue]        = React.useState(() => window.OMP_QUEUE.EMPTY_QUEUE);
  const [queueSending, setQueueSending] = React.useState([]);
  // omp's goal mode for this tab (app/goal.js `{known, goal}`).
  const [goal,         setGoal]         = React.useState(() => window.OMP_GOAL.UNKNOWN);
  // Status lines and text widgets the active tab's omp extensions published
  // (app/extension-widgets.js, #52), drawn as rail cards.
  const [extWidgets,   setExtWidgets]   = React.useState(_EXT_EMPTY);

  // ── Tab list — driven by bridge session registry ──────────────────────────
  // Each entry: { id, name, path, color, branch }
  const [sessions,        setSessions]        = React.useState([]);
  const [activeSessionId, setActiveSessionId] = React.useState("");

  // ── Profiles — every selectable omp profile ({id, name}) ─────────────────
  // The profile itself is per tab (see sessions[].profile); this is just the
  // catalogue the selector renders.
  const [profiles,        setProfiles]        = React.useState([]);
  // Which profile is ticked as the default for new tabs / the next launch.
  // App-wide and persisted, unlike a tab's own profile.
  const [startupProfileId, setStartupProfileId] = React.useState(DEFAULT_PROFILE_ID);
  // Recently opened folders of the active tab's profile (project sidebar).
  const [recentProjects, setRecentProjects] = React.useState([]);
  // With no tab open: the profile the next tab opens under (the menu's pick,
  // else the startup default — resolved by the bridge), and notes filed
  // while there was no transcript to hold them.
  const [noTabProfileId, setNoTabProfileId] = React.useState(DEFAULT_PROFILE_ID);
  const [workspaceNotes, setWorkspaceNotes] = React.useState([]);
  // Tool approval mode `{ default, projects }` (app/approval-mode.js), null
  // until the bridge's boot fetch lands.
  const [approvalModes, setApprovalModes] = React.useState(null);

  // ── Cross-cutting effects ─────────────────────────────────────────────────
  useBridgeSnapshot(bridge, {
    setMessages, setStreaming, setCtx, setSessionStats, setCtxBreakdown, setKanban, setPlanMeta,
    setModels, setActivity, setSparkline,
    setModelState, setThinkingLevel,
    setSessions, setActiveSessionId, setProfiles, setStartupProfileId, setRecentProjects,
    setNoTabProfileId, setWorkspaceNotes, setApprovalModes, setCloseConfirm,
    setPromptHistory, setSubagents, setSubagentTranscripts, setQueue, setQueueSending, setGoal, setExtWidgets,
  });
  useThemeEffect(t);
  // The mode picks the system prompt omp starts with (app_mode.rs).
  React.useEffect(() => { bridge?.setAppMode?.(t.appMode === "chat" ? "chat" : "code"); }, [bridge, t.appMode]);

  // Chat and Code live apart: a chat is a tab in the Chat folder (chat_space.rs),
  // everything else is a code project. Each mode remembers the tab it had open.
  const isChat = t.appMode === "chat";
  // Sidebar right-click actions: tabs marked unread (until opened), pinned and
  // archived conversations (by session file, so they outlive the tab).
  const [unread, setUnread] = React.useState(() => new Set());
  const pinned = t.pinnedSessions ?? [];
  const archived = t.archivedSessions ?? [];
  const handleMarkUnread = id => { if (id !== activeSessionId) setUnread(u => new Set(u).add(id)); };
  const handlePin = file => setTweak("pinnedSessions", pinned.includes(file) ? pinned.filter(f => f !== file) : [...pinned, file]);
  const handleReveal = path => { bridge?.revealPath?.(path); };
  // `row` of the chat sidebar ({ tab?, saved?, file }) or a code tab: the file is its saved conversation.
  const trashConversation = async (id, file) => {
    if (id) { await bridge?.closeSessionGuarded(id); }
    if (file) await bridge?.deleteSavedSession?.(file);
  };
  const handleArchiveRow = row => {
    if (row.file) setTweak("archivedSessions", [...archived, row.file]);
    if (row.tab) bridge?.closeSessionGuarded(row.tab.id);
  };
  const handleDeleteRow = row => { trashConversation(row.tab?.id, row.file); };
  // The interface-detail step as a class on <html> (layout/shell.css).
  React.useEffect(() => {
    const root = document.documentElement;
    const n = window.OMP_UI_DETAIL.clamp(t.uiDetail);
    for (let i = 1; i <= 5; i++) root.classList.toggle(`detail-${i}`, i === n);
    for (const w of ["narrow", "medium", "wide"]) root.classList.toggle(`chat-w-${w}`, (t.chatWidth ?? "narrow") === w);
  }, [t.uiDetail, t.chatWidth]);
  const [chatDir, setChatDir] = React.useState("");
  const [savedChats, setSavedChats] = React.useState([]);
  React.useEffect(() => { let live = true; Promise.resolve(bridge?.chatDir?.()).then(d => { if (live && d) setChatDir(d); }); return () => { live = false; }; }, [bridge]);
  const isChatTab = s => !!chatDir && s.path === chatDir;
  const chatTabs = sessions.filter(isChatTab);
  const codeTabs = sessions.filter(s => !isChatTab(s));
  React.useEffect(() => {
    if (!isChat || !chatDir) return undefined;
    let live = true;
    Promise.resolve(bridge?.listSavedSessions?.(chatDir)).then(l => { if (live && Array.isArray(l)) setSavedChats(l); });
    return () => { live = false; };
  }, [bridge, isChat, chatDir, chatTabs.length]);
  React.useEffect(() => {
    bridge?.setPromptHistoryLimit(t.promptHistoryLimit ?? window.OMP_PROMPT_HISTORY.DEFAULT_LIMIT);
  }, [bridge, t.promptHistoryLimit]);
  // Behaviours live.js owns (app/session-prefs.js): it takes its keys from
  // the tweaks and acts only on a changed one.
  React.useEffect(() => { bridge?.setSessionPrefs(t); }, [bridge, t]);
  // Refuse a file or link drop no handler took (`refusesStrayDrop`), so the
  // webview never navigates to it now that Tauri's native file drop is off
  // (#50). The composer's handlers run first and accept their own drops.
  React.useEffect(() => {
    const { refusesStrayDrop } = window.OMP_IMAGES;
    const { isTypingTarget } = window.OMP_KEYMAP;
    const refuse = e => {
      if (e.defaultPrevented) return;
      const el = e.target;
      const onEditable = el instanceof Element && isTypingTarget(el);
      if (!refusesStrayDrop(e.dataTransfer, onEditable)) return;
      e.preventDefault();
      if (e.type === "dragover") e.dataTransfer.dropEffect = "none";
    };
    window.addEventListener("dragover", refuse);
    window.addEventListener("drop", refuse);
    return () => {
      window.removeEventListener("dragover", refuse);
      window.removeEventListener("drop", refuse);
    };
  }, []);

  // Fetch OAuth providers whenever the login view opens (ensures fresh auth status)
  React.useEffect(() => {
    if (!bridgeOpen || bridgeView !== "login") return;
    setLoginProviders(null);
    bridge?.getLoginProviders()
      .then(data => setLoginProviders(data?.providers ?? []))
      .catch(() => setLoginProviders([]));
  }, [bridgeOpen, bridgeView]);

  const openBridge = view => { setBridgeView(view); setBridgeOpen(true); };

  // ── Derived values ────────────────────────────────────────────────────────
  const activeProject = sessions.find(s => s.id === activeSessionId) ?? sessions[0] ?? EMPTY_PROJECT;
  // No tab open → `activeProject` is EMPTY_PROJECT (built-in profile), but
  // the bridge spawns the next tab into the empty workspace's pick or the
  // ticked startup profile (nothing to inherit from) — so showing the
  // built-in would tick a profile that isn't where the next tab actually
  // goes. Shared by WindowChrome's menu, the keymap, recents and the
  // usage-stats panel's header label, instead of a second copy.
  const activeProfileId    = activeProject.id ? activeProject.profile : noTabProfileId;
  const activeProfileLabel = profiles.find(p => p.id === activeProfileId)?.name ?? activeProfileId;
  // Status-bar approvals cell: the mode the rendered tab's omp runs, or
  // `running ⇢ target` while a change waits for its turn to end. Read
  // late-bound: app/approval-mode.js is a plain module like the others.
  const approvalCell = window.OMP_APPROVAL_MODE.statusCell(approvalModes, activeProject.id ? activeProject : null);
  // The tab being rendered owns the plan state: keyed on `activeProject.id`
  // for the same reason as `handleSelectProfile` below, and "" with no tab
  // open, which `updateEntry` ignores.
  const planKey = activeProject.id;
  const plan    = entryOf(plans, planKey, PLAN_IDLE);
  const updatePlan = React.useCallback(
    fn => setPlans(m => updateEntry(m, planKey, PLAN_IDLE, fn)), [planKey]);
  const handleAnnotate = React.useCallback(
    (idx, value) => updatePlan(p => annotatePlan(p, idx, value)), [updatePlan]);
  // "rail" (the idle entry, also with no tab open), "tree" or "browser".
  const side = entryOf(sides, planKey, "rail");
  const setSide = React.useCallback(
    next => setSides(m => updateEntry(m, planKey, "rail", () => next)), [planKey]);
  const toggleSide = kind => setSide(side === kind ? "rail" : kind);
  const setBrowserOpen = React.useCallback(
    on => setSides(m => updateEntry(m, planKey, "rail", s => (on ? "browser" : s === "browser" ? "rail" : s))), [planKey]);
  const tabCollapseOverrides = entryOf(collapseOverrides, planKey, NO_COLLAPSE_OVERRIDES);
  const handleToggleCollapse = React.useCallback(
    (id, collapsed) => setCollapseOverrides(m => updateEntry(m, planKey, NO_COLLAPSE_OVERRIDES, o => ({ ...o, [id]: !collapsed }))),
    [planKey]);
  const foldModes = _foldModes(t);
  // Stable per tab: ToolCard is memoised, an inline closure would re-render
  // every card. omp reports a tool path relative to the tab's folder, Rust
  // reads a relative path as repo-root-relative, so it goes over absolute.
  const gitProject = activeProject.path ?? null;
  const handleOpenFile = React.useCallback(
    path => setGitOpen({ path: _gitAbsolutePath(gitProject, path), project: gitProject }),
    [gitProject]);
  const closeGit = React.useCallback(() => setGitOpen(null), []);
  // A read card's path: the preview, bound to this tab.
  const handlePreviewFile = React.useCallback(
    target => setPreview({ key: planKey, target }), [planKey]);
  const closePreview = React.useCallback(() => setPreview(null), []);
  const previewToGit = React.useCallback(path => { setPreview(null); handleOpenFile(path); }, [handleOpenFile]);
  // The git browser refetches when this count rises; counted only while it
  // is open, and handed over as a number so streaming frames that finish no
  // edit leave the memoised browser alone.
  const gitEdits = React.useMemo(() => (gitOpen ? _gitFinishedEdits(messages) : 0), [gitOpen, messages]);
  // Collapse all / Expand all: one toggle per block in this tab, replacing
  // the tab's earlier ones; later blocks follow the modes again.
  const handleCollapseAll = (collapsed) => setCollapseOverrides(m => updateEntry(m, planKey, NO_COLLAPSE_OVERRIDES,
    () => _collapseOverridesForAll(messages, collapsed, { hidden: window.OMP_TURN_STATUS.isHidden })));
  const openSettings = section => setStab(s => window.OMP_SETTINGS_TAB.open(s, section));
  const closeSettings = () => setStab(window.OMP_SETTINGS_TAB.close);
  // Another omp tab shown (opened, picked, restored) leaves the settings tab.
  React.useEffect(() => { setStab(window.OMP_SETTINGS_TAB.selectProject); setHome(false); }, [activeSessionId]);
  // Tab ids, for dropping the per-tab state of closed tabs (here and in
  // the composer's drafts). live.js rebuilds `sessions` on every snapshot,
  // streaming deltas included, so the list is memoised on the ids' content:
  // the prune effects run only when a tab opens or closes.
  const sessionIdsKey = sessions.map(s => s.id).join("\0");
  const sessionIds = React.useMemo(() => (sessionIdsKey ? sessionIdsKey.split("\0") : []), [sessionIdsKey]);
  // A tab seen for the first time starts with the `defaultSide` setting;
  // tabs already open keep theirs when the setting changes.
  const seenSideIds = React.useRef(new Set());
  React.useEffect(() => {
    const fresh = sessionIds.filter(id => !seenSideIds.current.has(id));
    seenSideIds.current = new Set(sessionIds);
    setPlans(m => pruneEntries(m, sessionIds));
    setCollapseOverrides(m => pruneEntries(m, sessionIds));
    setSides(m => seedEntries(pruneEntries(m, sessionIds), fresh, "rail", t.defaultSide ?? "rail"));
  }, [sessionIds]);
  // Profile chip text for a tab/project: none for the built-in profile (same
  // expression as TabBar's own copy, which labels the tab bar).
  const profileLabel = id =>
    id === DEFAULT_PROFILE_ID ? null : (profiles.find(p => p.id === id)?.name ?? id);
  const todoCounts    = kanban.reduce(
    (acc, col) => {
      acc.total += col.tasks.length;
      acc.done  += col.tasks.filter(tk => tk.status === "done").length;
      return acc;
    },
    { total: 0, done: 0 }
  );

  // ── Keymap ────────────────────────────────────────────────────────────────
  // The derived profile, not the tab's own: with no tab open it follows the
  // startup default / empty-workspace pick, which must reload the omp layer.
  const keymap = useKeymap(bridge, activeProfileId);

  // One shell per tab, in a panel above the composer (app/use-terminal.jsx).
  const terminal = useTerminal({
    bridge, activeSessionId: planKey, sessionIds, tweaks: t, setTweak,
  });

  const subagentUi = useSubagentManager({
    bridge, layout: t.layout, setTweak, activeSessionId, subagents, messages, setHoveredMsgIdx,
  });
  const subagentList = React.useMemo(() => window.OMP_SUBAGENTS.listAgents(subagents), [subagents]);


  // The browser panel (app/use-browser.jsx). `activeProject.id`, like the
  // widgets below, not `activeSessionId`, which keeps a closed tab's id.
  const browser = useBrowser({
    bridge, activeSessionId: activeProject.id, sessionIds, open: side === "browser", setOpen: setBrowserOpen, tweaks: t, setTweak,
  });
  // Clicks on links in the agent's markdown: a project file opens the
  // preview (Ctrl/⌘-click: the git browser); anything else goes on to the
  // browser panel's localhost handler. Without this a relative link would
  // navigate the webview, which navigation_guard.rs silently blocks.
  const browserLinkClick = browser.onChatClick;
  const handleChatLinkClick = React.useCallback(e => {
    const file = _fpLinkClick(e, gitProject);
    if (!file) { browserLinkClick?.(e); return; }
    e.preventDefault();
    if (file.git) handleOpenFile(file.target.path);
    else handlePreviewFile(file.target);
  }, [browserLinkClick, gitProject, handleOpenFile, handlePreviewFile]);
  const stageRef = React.useRef(null);
  // A chip click scrolls the panel to the annotated element — but only
  // when the panel is open and still on the page the chip was picked from
  // (`browser.state.url` is the hook's current page, `planKey` the chips'
  // own tab). A pick from another page has no rect there; a closed panel
  // has no page to scroll (the bridge call is quiet anyway).
  const handleBrowserChipClick = (chip) => {
    if (!browser.open || !chip?.pick?.docRect || chip.pick.url !== browser.state.url) return;
    bridge?.browserScrollTo(planKey, chip.pick.docRect);
  };
  // `/browser [url]`: open the panel, load a page, or answer omp's
  // `headless|visible` with a note (OMP_BROWSER.browserCommand).
  const handleBrowserCommand = (args) => {
    const r = _browserCommand(args);
    if (r.open) setBrowserOpen(true);
    else if (r.url) browser.openUrl(r.url);
    else bridge?.addAssistantMessage(r.note);
  };

  // `activeProject.id`, not `activeSessionId`: the latter keeps a closed
  // tab's id (see handleSelectProfile below). Widgets only run, and the PR
  // card only reads with Auto-fix off, while the rail is on screen — not in
  // focus layout, not under the tree or browser.
  const railVisible = t.layout !== "focus" && side === "rail";
  const widgets = useWidgets({
    bridge, activeSessionId: activeProject.id, activeProjectPath: activeProject.path ?? null, railVisible, tweaks: t, setTweak,
  });
  // Pushed by omp, not run by the desktop: nothing to schedule, only a
  // Settings → Rail toggle per card (`widget:ext-status`, `widget:ext:<key>`).
  const extCards = React.useMemo(() => _extCards(extWidgets), [extWidgets]);
  const extToggles = React.useMemo(() => _extToggles(extWidgets), [extWidgets]);

  // Handler map read through a ref so the dispatch effect never re-subscribes.
  const handlersRef = React.useRef({});
  useKeymapDispatch(handlersRef);

  // ── Handlers ──────────────────────────────────────────────────────────────
  // Where a submit goes (OMP_SLASH.routeSubmit): decided on the typed draft.
  const routeOf = (msg, followUp) => _routeSubmit(data.commands, msg, {
    goalMode: goalUi.mode, followUp, streaming, connected: Boolean(bridge?.isConnected),
  });
  // The tab's browser annotations go out with a submit like the rest of the
  // draft: taken at once (chips added since stay for the next message) and
  // merged after the typed text and images (OMP_BROWSER.mergeIntoMessage:
  // image cap, page data fenced and neutralised, so the merge never turns a
  // message into a command). A command takes none: they wait for the next
  // message. Returns the merge plus the chips taken (`chips`), which a
  // failed goal start hands back with its draft.
  const takeBrowserChips = (text, images, route) => {
    const chips = route.isCmd ? [] : browser.chips;
    if (chips.length) browser.takeChips(chips.map(c => c.id));
    return { ..._mergeBrowserChips(text, images ?? [], chips), chips };
  };
  const startGoal = (msg, images, route) => {
    const out = takeBrowserChips(msg, images, route);
    goalUi.start({ text: msg, images, chips: out.chips }, out);
  };
  const handleSend = (text, images) => {
    const hasAnnotations = Object.keys(plan.annotations).length > 0;
    if (!text.trim() && !hasAnnotations && !(images && images.length > 0) && !browser.chips.length) return;
    let msg = text.trim();
    // A slash command (typed by hand, or inserted by picking a non-desktop
    // entry from the `/` popup or the ⌘K bridge) must reach omp with its
    // leading `/` at position 0, or omp's dispatcher never recognizes it as
    // a command — it just becomes ordinary prose. Checked against the real
    // command list, not just "starts with /": plan-mode prose routinely
    // starts with a path ("/etc/nginx.conf is wrong"), which both
    // plan-mode rewrites below would otherwise skip by mistake. A skill
    // invoked mid-prompt ("review this /skill:code-review", #30) counts as
    // a command too: it is sent as typed, like a leading `/skill:x`.
    // `routeOf` classifies the typed draft, before any page data merges.
    const route = routeOf(msg, false);
    // Goal mode (app/use-goal-mode.jsx): the typed text is the goal's
    // objective. Plan mode cannot be on at the same time; a command still
    // runs as one. Browser annotations ride along in the goal's first
    // prompt (not in the stored objective); a refusal hands the typed
    // draft back, chips included.
    if (route.via === "goal") {
      startGoal(msg, images, route);
      return;
    }
    if (plan.mode && !route.isCmd) {
      if (hasAnnotations) {
        // Feedback with block comments — always takes priority over intent framing
        const lineComments = Object.entries(plan.annotations)
          .sort(([a], [b]) => Number(a) - Number(b))
          .map(([, { raw, comment }]) => {
            const quoted = raw.split('\n').map(l => `> ${l}`).join('\n');
            return `${quoted}\n→ ${comment.trim()}`;
          }).join('\n\n');
        const parts = ['Line comments:\n' + lineComments, text.trim()].filter(Boolean);
        msg = parts.join('\n\n');
        updatePlan(markPlanSent); // annotations imply plan is already in progress; spends them
      } else if (!plan.started) {
        // First clean send — wrap in intent framing. trimEnd() matters for an
        // image-only send (empty text): INTENT_FRAMING's template ends in a
        // literal "\n\n" that .trim() on the intent argument never touches,
        // and the server's message_start echo arrives already .trim()med
        // (live.js) — an untrimmed local echo would fail that dedup compare
        // and double-render the bubble.
        updatePlan(markPlanSent);
        msg = INTENT_FRAMING(text.trim()).trimEnd();
      }
    }
    // Browser annotations join the message last, after plan framing, so only
    // what was typed is framed.
    const out = takeBrowserChips(msg, images, route);
    if (route.via === "steer") bridge?.steer(out.text, out.images);
    else if (route.via === "prompt") bridge.send(out.text, out.images);
    else setMessages(prev => [...prev, { kind: "user", time: timeNow(), text: out.text, images: out.images }]);
  };

  const handleAbort      = () => { prAutomation.pause(); bridge?.abort(); setStreaming(false); };
  const handlePickModel  = m  => { setModelState(m); bridge?.setModel(m); };
  const handleAskAnswer  = React.useCallback((id, value) => { bridge?.answerAsk(id, value); }, [bridge]); // bridge = window.OMP_BRIDGE, assigned once before React renders — stable ref
  const handleAskDialogAnswer = React.useCallback((id, answers) => { bridge?.answerAskDialog(id, answers); }, [bridge]);
  const handleConfirmAsk = React.useCallback((id, confirmed) => { bridge?.answerConfirm(id, confirmed); }, [bridge]);
  const handleCancelAsk  = React.useCallback((id) => { bridge?.cancelAsk(id); }, [bridge]);
  const handleGrantApproval = React.useCallback((prompt, scope) => { bridge?.approveAndRemember(prompt, scope); }, [bridge]);
  const handleStopRetry  = () => { prAutomation.pause(); bridge?.stopRetry(); };
  const handlePickLogin = async (provider) => {
    if (!bridge) return;
    try {
      // OMP_BRIDGE.login resolves when OAuth completes (≤300 s).
      // live.js handles extension_ui_request.open_url via open_url_external (system browser).
      // For already-authenticated providers omp refreshes the token silently (no browser).
      // Use bridge.addAssistantMessage — writes into state.messages so the message
      // survives any subsequent notify() call (e.g. model registry refresh after login).
      await bridge.login(provider.id);
      bridge.addAssistantMessage(`Logged in to **${provider.name}**.`);
    } catch (err) {
      const msg = err?.message ?? String(err);
      bridge.addAssistantMessage(`**Login failed (${provider.name}):** ${msg}`);
    }
  };
  const cycleThinking    = () => bridge?.cycleThinking();

  // Extracted so both the global keymap handler and the composer prop share
  // the same implementation (plan §7: "extract into a togglePlanMode callback").
  // Plan mode and a goal exclude each other, as in omp: the toggle only
  // turns plan mode off while the tab has a goal (or is in goal mode).
  const togglePlanMode = () => {
    if (!plan.mode && goalUi.planBlocked) return;
    updatePlan(togglePlan);
  };

  // Composer-scoped follow-up: the composer owns the draft text. Re-trim
  // here (mirrors handleSend's `msg = text.trim()`) — expandPastes runs
  // after the composer's own trim and can reintroduce leading/trailing
  // whitespace from the raw pasted content. In goal mode the follow-up
  // chord starts the goal too, like Enter (handleSend). The tab's browser
  // annotations ride along as they do with a send.
  const handleFollowUp = (text, images) => {
    const msg = text.trim();
    const route = routeOf(msg, true);
    if (route.via === "goal") {
      startGoal(msg, images, route);
      return;
    }
    const out = takeBrowserChips(msg, images, route);
    bridge?.followUp(out.text, out.images);
  };
  // Queue strip actions: resolve `true` once omp edited its queue.
  const handleRemoveQueued  = (text, kind) => bridge?.removeQueuedMessage(text, kind);
  const handlePromoteQueued = text => bridge?.promoteQueuedMessage(text);
  const handleSendQueuedNow = (text, kind) => bridge?.sendQueuedNow(text, kind);

  // CommandBridge (⌘K) picking a non-desktop (RPC) command — no local
  // handler exists for it, unlike handleCommand's desktop entries. It has
  // no textarea of its own, so it hands the `/name ` draft up here for the
  // Composer to pick up and focus; the user fills in arguments and Enter
  // sends it as a normal prompt, which omp executes.
  const [draftInsert, setDraftInsert] = React.useState(null);
  const handleInsertDraft = (text) => setDraftInsert({ text, nonce: Date.now() });

  // Prompt-history picker (Ctrl+Up modal, issue #16) — same {text, nonce}
  // shape and rationale as draftInsert above, but *replaces* the draft (see
  // composer.jsx's promptInsert effect) rather than prepending. Rail widget
  // inserts set `append` instead: they go after the draft
  // (OMP_WIDGETS.appendToDraft), since ⌘K's draftInsert replaces a
  // `/command` draft.
  const [promptInsert, setPromptInsert] = React.useState(null);
  const handlePickHistoryPrompt = (text) => setPromptInsert({ text, nonce: Date.now() });
  // `action` (OMP_WIDGETS.insertAction, from widgets.json only): "new-tab"
  // always opens a fresh tab on the project and fills that tab's draft (never
  // sent); a failed open is reported by openSession and inserts nothing.
  const insertOpeningRef = React.useRef(false);
  const handleWidgetInsert = React.useCallback(async (text, action) => {
    if (action?.target === "new-tab" && bridge) {
      // No overlapping opens: a double-click or key repeat is dropped
      // while one open is in flight; a click after it settles opens
      // another tab.
      if (insertOpeningRef.current) return;
      insertOpeningRef.current = true;
      try {
        const id = await bridge.openSession(action.project);
        setPromptInsert({ text, append: true, sessionId: id, nonce: Date.now() });
      } catch (err) {
        console.error("[widgets] failed to open insert target tab:", err);
      } finally {
        insertOpeningRef.current = false;
      }
      return;
    }
    setPromptInsert({ text, append: true, nonce: Date.now() });
  }, [bridge]);

  // Goal mode in the composer and the goal strip above it (omp's `goal`).
  const goalUi = useGoalMode({
    bridge, tabId: planKey, sessionIds, goal, planMode: plan.mode, profileId: activeProfileId,
    runState: activeProject.runState,
    // The chips go back to the tab they were taken from whatever tab is
    // shown; the text only into the composer of that tab still shown.
    onRestoreDraft: ({ text, images, chips }, { tabId, shown }) => {
      if (shown) setPromptInsert({ text, images, nonce: Date.now() });
      browser.restoreChips(chips ?? [], tabId);
    },
  });

  const prAutomation = usePrAutomation({
    bridge, tabId: planKey, sessionIds, projectKey: activeProject.path,
    runState: activeProject.runState,
    branchKey: activeProject.branch ?? null,
    conversationKey: JSON.stringify([activeProject.profile, activeProject.sessionFile ?? null]),
    featureEnabled: t.showChanges ?? true,
    railVisible,
    // Disconnected, stabilising or folder-less tabs are the hook's own
    // `connected`/`projectKey` gates.
    connected: !!bridge?.isConnected && !stab.open,
    blocked: plan.mode || goalUi.mode || !!goalUi.planBlocked ||
      window.OMP_TURN_STATUS.busyElsewhere(sessions, planKey, activeProject.path),
  });

  // Conversation tree data + actions. A successful "branch here" puts the
  // re-asked prompt into the composer through `promptInsert` (replaces the
  // draft). `sessionFile` comes with every snapshot's tab entry: it changes
  // when branch/fork/new move the process onto another file, which refetches.
  const convTree = useConversationTree({
    bridge, open: side === "tree", sessionId: activeProject.id, sessionFile: activeProject.sessionFile ?? null,
    profileId: activeProfileId, processStartedAt: activeProject.processStartedAt ?? null,
    streaming, onBranchText: text => setPromptInsert({ text, nonce: Date.now() }),
  });

  // `args`: text typed after a desktop command in the composer (composer.jsx
  // `execCmd`); empty from the ⌘K bridge.
  const handleCommand = (c, args = "") => {
    if      (c.name === "plan")      { if (!goalUi.planBlocked) updatePlan(enterPlan); }
    // `/goal <objective>`: goal mode with the objective back in the
    // composer, budget still to pick; Enter starts it. Blocked (plan mode,
    // a goal already open), the text still comes back.
    else if (c.name === "goal")      { goalUi.enter(); if (args) setPromptInsert({ text: args, nonce: Date.now() }); }
    else if (c.name === "todo")      { setPlanOpen(true); }
    else if (c.name === "browser")   { handleBrowserCommand(args); }
    else if (c.name === "compact")   { bridge?.compact(); }
    else if (c.name === "tree" || c.name === "branch") { setSide("tree"); }
    else if (c.name === "export")    { bridge?.exportHtml(); }
    else if (c.name === "thinking")  { cycleThinking(); }
    else if (c.name === "model")     { openBridge("models"); }
    else if (c.name === "login")     { openBridge("login"); }
    else if (c.name === "new")       { bridge?.newSession(); }
    else if (c.name === "history")   { setHistoryOpen(true); }
    else if (c.name === "shortcuts") { openSettings("advanced"); }
    else if (c.name === "collapse-all")  { handleCollapseAll(true); }
    else if (c.name === "expand-all")    { handleCollapseAll(false); }
    else if (c.name === "settings")  { openSettings(); }
    else if (c.name === "terminal")  { terminal.toggle(planKey); }
  };

  const handleResumeSession = async (session) => {
    if (!bridge || !session) return;
    try {
      await bridge.resumeSession(session);
    } catch (err) {
      // Same dangling-profile rejection as openSession (see handleNewProject
      // below) — the bridge already rolled back its ghost tab registration.
      console.error("[app] failed to resume session:", err);
    }
  };

  const handleApprovePlan = () => {
    updatePlan(approvePlan);
    bridge?.followUp(APPROVAL_PROMPT);
    setPlanOpen(true);
  };

  // Tab select — switches the active session; bridge resets all per-session state
  // and re-fetches from the new session's omp → notify() pushes fresh data.
  const handleSelectTab = id => {
    setUnread(u => { if (!u.has(id)) return u; const n = new Set(u); n.delete(id); return n; });
    setHome(false);
    setStab(window.OMP_SETTINGS_TAB.selectProject);
    if (id === activeSessionId) return;
    bridge?.activateSession(id);
    // setActiveSessionId is driven by snap.activeSessionId from onUpdate
  };

  // Open project → new session → new tab with its own omp process
  const handleNewProject = async () => {
    if (!bridge) return;
    const path = await bridge.pickFolder();
    if (!path) return;
    try {
      await bridge.openSession(path);
      // Tab list and activeSessionId are updated via onUpdate from the bridge
    } catch (err) {
      // openSession rejects when the inherited/ticked profile no longer
      // exists (deleted by another window, or a hand-edited profiles.json)
      // and the backend refuses to spawn under it — the bridge already
      // rolled back the ghost tab registration, so there's nothing left to
      // undo here; just keep the rejection from going unhandled.
      console.error("[app] failed to open project:", err);
    }
  };

  // The welcome page: open `path` as a tab, then send the first message.
  const handleStartFromWelcome = async (path, text, opts = {}) => {
    if (!bridge) return;
    try {
      if (isChat && path === chatDir) await bridge.openSession(path); else await bridge.openProject(path);
      setHome(false);
      await bridge.waitForActivation?.();
      // The choices made on the page, applied to the new tab before the first
      // message goes out. A model equal to omp's default is not re-sent.
      if (opts.model) { setModelState(opts.model); bridge.setModel(opts.model); }
      if (opts.level) { try { await bridge.setThinkingLevel(opts.level); } catch { /* the model keeps its own level */ } }
      await bridge.send(text, opts.images?.length ? opts.images : undefined);
    } catch (err) {
      console.error("[app] failed to start from the welcome page:", err);
    }
  };

  // Close tab → kills that session's omp process; bridge updates tab list.
  // Guarded: with `confirmBusyClose` a working tab asks first (live.js).
  // Rewind: back to before a message, its text returned to the prompt box.
  const handleRewind = async (msg, dup) => {
    const res = await bridge?.rewindToPrompt?.(msg.text, dup);
    if (res?.ok) setPromptInsert({ text: res.text, nonce: Date.now() });
    else if (res && !res.cancelled) console.warn("[app] rewind failed:", res.error);
  };
  const handleCloseTab = id => { bridge?.closeSessionGuarded(id); };
  // Manual rename (#32): omp's `/rename <title>` via the bridge, which
  // activates a background tab first and keeps the transcript clean.
  const handleRenameTab = (id, title) => { bridge?.renameSession(id, title); };

  // ── Project navigation (#27) ──────────────────────────────────────────────
  const showSidebar   = t.sidebar ?? true;
  const toggleSidebar = () => setTweak("sidebar", !showSidebar);

  // A recent project: focus its open tab in the active profile, else spawn.
  const handleOpenRecent = async path => {
    if (!bridge) return;
    try {
      await bridge.openProject(path);
    } catch (err) {
      // Same rejection/rollback as openSession (see handleNewProject).
      console.error("[app] failed to open recent project:", err);
    }
  };

  // "New conversation" in a project: always a new tab, under the project's
  // own profile rather than the active tab's.
  const handleNewInProject = async (path, profile) => {
    if (!bridge) return;
    try {
      await bridge.openSession(path, profile);
    } catch (err) {
      console.error("[app] failed to open conversation:", err);
    }
  };

  const handleForgetRecent = path => { bridge?.forgetRecentProject(path); };

  // Drag to reorder (#40), from the tab bar or the sidebar: `onDrop` of
  // `useDragReorder`, in display order. Member lists show newest first, the
  // registry's reverse, so a drop above a row is a registry move behind it.
  const handleReorder = (list, id, targetId, after) => {
    const order = list === "projects"
      ? moveGroup(sessions, id, targetId, after)
      : moveMember(sessions, id, targetId, !after);
    if (order) bridge?.reorderTabs(order);
  };
  // Keyboard move: the active tab's project, one place along the tab bar.
  const shiftActiveProject = delta => {
    if (!activeProject.id) return;
    const order = shiftGroup(sessions, groupKey(activeProject), delta);
    if (order) bridge?.reorderTabs(order);
  };
  // Ctrl+Tab / Ctrl+Shift+Tab over the project tabs and, when open, the
  // settings tab as the last stop. Indexed on `activeProject.id`, not
  // `activeSessionId`: `activeProject` falls back to `sessions[0]` when the
  // snapshot's `activeSessionId` is stale or empty.
  const cycleTabs = dir => {
    const r = window.OMP_SETTINGS_TAB.cycle(window.OMP_SETTINGS_TAB.initial(), sessionIds, activeProject.id, dir);
    if (r.settingsActive) { openSettings(); return; }
    setStab(window.OMP_SETTINGS_TAB.selectProject);
    if (r.sessionId && r.sessionId !== activeProject.id) bridge?.activateSession(r.sessionId);
  };

  // ── Global keymap handler map ─────────────────────────────────────────────
  // Written into handlersRef every render so the dispatch hook always reads
  // the latest closures without re-subscribing to the window listener.
  // Placed after handleNewProject/handleCloseTab so every entry can be a
  // direct function reference (matching cycleThinking/togglePlanMode above)
  // instead of a `() => handleNewProject()` wrapper that only exists to
  // dodge a TDZ that a closure never actually hits.
  handlersRef.current = {
    "app.interrupt":           () => {
      // Only abort when no overlay is open (overlays handle Escape themselves),
      // and never from the Settings page, which covers the chat.
      if (stab.open || bridgeOpen || historyOpen || gitOpen || rulesOpen || statsOpen || promptHistoryOpen || planOpen) return;
      if (streaming) handleAbort();
    },
    "app.thinking.cycle":      cycleThinking,
    "app.model.cycleForward":  () => bridge?.cycleModel(),
    "app.model.cycleBackward": () => {
      if (models.length < 2) return;
      const idx = models.findIndex(m => m.id === model.id && m.provider === model.provider);
      // No-op when the current model isn't in the list (plan §7).
      if (idx < 0) return;
      handlePickModel(models[(idx - 1 + models.length) % models.length]);
    },
    "app.model.select":        () => openBridge("models"),
    "app.plan.toggle":         togglePlanMode,
    "app.session.new":         () => bridge?.newSession(),
    "app.session.resume":      () => setHistoryOpen(v => !v),
    "desktop.commands.open":   () => { setBridgeView("commands"); setBridgeOpen(v => !v); },
    "desktop.history.open":    () => setHistoryOpen(v => !v),
    "desktop.shortcuts.open":  () => openSettings("advanced"),
    "desktop.settings.open":   () => (stab.open ? closeSettings() : openSettings()),
    "desktop.sidebar.toggle":  toggleSidebar,
    "desktop.tab.new":         handleNewProject,
    // The settings tab, when it is the one shown, closes itself; it is the
    // last stop of next/prev (OMP_SETTINGS_TAB.cycle).
    "desktop.tab.close":       () => {
      if (stab.open) closeSettings();
      else if (activeProject.id) handleCloseTab(activeProject.id);
    },
    "desktop.tab.next":        () => cycleTabs(1),
    "desktop.tab.prev":        () => cycleTabs(-1),
    "desktop.tab.moveLeft":    () => shiftActiveProject(-1),
    "desktop.tab.moveRight":   () => shiftActiveProject(1),
    "desktop.panel.todo":      () => setPlanOpen(v => !v),
    "desktop.panel.changes":   () => setGitOpen(v => (v ? null : { path: null })),
    "desktop.panel.rules":     () => setRulesOpen(v => !v),
    "desktop.panel.stats":     () => setStatsOpen(v => !v),
    "desktop.composer.promptHistory": () => setPromptHistoryOpen(v => !v),
    "desktop.session.compact": () => bridge?.compact(),
    "desktop.session.export":  () => bridge?.exportHtml(),
    "desktop.chat.collapseAll": () => handleCollapseAll(true),
    "desktop.chat.expandAll":  () => handleCollapseAll(false),
    "desktop.terminal.toggle": () => terminal.toggle(planKey),
    "desktop.browser.toggle":  () => toggleSide("browser"),
  };

  // Profile switch applies to the active tab only: its omp process is
  // respawned under the new profile (a live process can't be moved to a
  // different auth/session tree), so the tab comes back as a fresh session.
  //
  // Dispatched on `activeProject.id` - the tab the menu is *rendering* - not
  // on `activeSessionId`. The two diverge because `use-bridge-snapshot`
  // only assigns `activeSessionId` when the snapshot's value is truthy, so a
  // `null` from the bridge leaves the closed id in React state; and right
  // after a tab click `activeSessionId` still names the previous tab, which
  // would respawn a different tab than the one shown (and on that
  // `wasActive === false` path a failure would surface no note at all).
  const handleSelectProfile = React.useCallback(id => {
    // EMPTY_PROJECT.id is "" when no tab is open — the app starts that
    // way — so there is no process to respawn: the pick only decides which
    // profile the next tab opens under (and whose recents are listed).
    if (!activeProject.id) return bridge?.selectNoTabProfile(id);
    return bridge?.switchSessionProfile(activeProject.id, id);
  }, [bridge, activeProject.id]);
  const handleCreateProfile = React.useCallback(name => bridge?.createProfile(name), [bridge]);
  const handleRenameProfile = React.useCallback((id, name) => bridge?.renameProfile(id, name), [bridge]);
  // Returns {ok, error}: the bridge refuses to delete a profile any open tab
  // still runs under, and the menu renders that reason inline.
  const handleDeleteProfile = React.useCallback(id => bridge?.deleteProfile(id), [bridge]);

  // Returns {ok, error}; the backend re-validates the id under its lock, so
  // the menu renders a refusal inline rather than assuming success.
  const handleSetStartupProfile = React.useCallback(id => bridge?.setStartupProfile(id), [bridge]);
  // Stable: the dialog's key listener re-subscribes on a new handler.
  const handleCloseAnswer = React.useCallback(ok => bridge?.resolveCloseConfirm(ok), [bridge]);

  const showRail  = t.layout !== "focus";
  const showSplit = subagentUi.paneOpen;
  const liveCtx   = ctx ?? data.ctx;
  // No tab open (the app starts that way): the session column shows the
  // empty state instead of a transcript and a composer with nowhere to send.
  const noTab = sessions.length === 0;
  const activeIsChat = isChatTab(activeProject);
  // The other mode's tab is not shown: this mode's start page is.
  const showHome = noTab || home || (sessions.length > 0 && activeIsChat !== isChat);
  // One artwork behind the whole column, the prompt box included.
  const sessionWallpaper = t.wallpaper === false ? null
    : window.OMP_WALLPAPERS.pick(t.wallpaperArt, showHome ? `home-${new Date().toDateString()}` : activeSessionId).file;
  // Switching modes always lands on the start page of the mode.
  const handleMode = m => {
    if (m === t.appMode) return;
    setTweak("appMode", m);
    setHome(true);
  };
  const quotas = useQuotas({ bridge, profileId: activeProfileId, enabled: showRail && !showHome && side !== "browser" });
  const welcomeProjects = (() => {
    const seen = new Set();
    const rows = [];
    const add = path => { if (path && !seen.has(path)) { seen.add(path); rows.push({ path }); } };
    add(activeProject.path);
    sessions.forEach(s => add(s.path));
    recentRows(recentProjects, sessions, activeProfileId).forEach(r => add(r.path));
    return rows;
  })();

  // Resizable columns beside the chat (#41): which are shown, and their
  // widths as CSS variables on `.workspace`. Browser full view hides the
  // chat and the split pane (layout/browser.css): only the sidebar is left.
  const workspaceRef = React.useRef(null);
  const browserFull = side === "browser" && browser.fullView;
  const splitShown = showSplit && !browserFull;
  // No side column on the welcome page: there is no session to describe.
  const sideRail = showHome ? null : side === "tree" ? "tree" : side !== "browser" && showRail ? "rail" : null;
  const termShown = !showHome && !browserFull && t.appMode !== "chat" && t.appMode !== "chat" && _TERM_entryOf(terminal.state, planKey).open;
  // Columns after the chat, left to right: the side panel (rail / tree /
  // browser), the subagent pane, the terminal; a new one opens at the far right.
  const stageCols = ["minmax(0, 1fr)"];
  if (!browserFull) {
    if (side === "browser") stageCols.push("auto");
    else if (sideRail) stageCols.push(sideRail === "tree" ? "var(--w-tree, min(540px, 50%))" : "var(--w-rail, 280px)");
    if (splitShown) stageCols.push("var(--w-split, 320px)");
  }
  if (termShown) stageCols.push("var(--w-term, 480px)");
  const stageStyle = { gridTemplateColumns: stageCols.join(" ") };
  // The grid line each handle sits at: the column it is the left edge of.
  const hasSide = !browserFull && (side === "browser" || !!sideRail);
  const splitCol = hasSide ? 3 : 2;
  const termCol = 2 + (hasSide ? 1 : 0) + (splitShown ? 1 : 0);
  const panes = usePaneWidths({
    tweaks: t, setTweak, workspaceRef,
    open: [showSidebar && "sidebar", splitShown && "split", sideRail, termShown && "term"].filter(Boolean),
    reserve: side === "browser" && !browserFull ? _AL_BROWSER_MIN : 0,
  });

  // The widget switch keys the Settings Rail section renders: badges and
  // resets count only these (a removed plugin's stored key has no row).
  const settingsKeyOpts = React.useMemo(() => ({
    prefixKeys: [...widgets.togglePlugins.map(p => _widgetTweakKey(p.id)), ...extToggles.map(r => r.tweak)],
  }), [widgets.togglePlugins, extToggles]);

  // What the Settings page reads and writes (design/settings/). Built only
  // while the tab is shown: behind a project tab the page stays mounted
  // with the last one (`active: false`), so streaming frames re-render
  // nothing there.
  const settingsCtx = stab.open ? {
    active: true, t, setTweak, bridge, keymap,
    profiles, startupProfileId, onSetStartupProfile: handleSetStartupProfile,
    approvalModes, sessions, activeProject,
    widgets, widgetTweakKey: _widgetTweakKey, extToggles, keyOpts: settingsKeyOpts,
    panes, terminalHeight: terminal.height,
    goal: { profileId: activeProfileId },
    openApprovals: () => setRulesOpen(true),
    openStats: () => { closeSettings(); setStatsOpen(true); },
    onLogin: handlePickLogin,
    profileId: activeProfileId,
    section: stab.section,
    close: closeSettings,
    setSection: id => setStab(s => window.OMP_SETTINGS_TAB.setSection(s, id)),
  } : null;

  // The "⋯" menu in the top bar: everything that is not a daily action.
  const keyHint = id => window.OMP_KEYMAP?.hintFor(id) ?? "";
  const floatingOn = t.layout !== "focus";
  const moreItems = [
    { label: "Todo and plan", icon: "plan", hint: todoCounts.total > 0 ? `${todoCounts.done}/${todoCounts.total}` : "", onClick: () => setPlanOpen(true) },
    { label: "Conversation tree", icon: "branch", on: side === "tree", onClick: () => toggleSide("tree") },
    { label: "Goal mode", icon: "goal", on: goalUi.mode, onClick: () => { if (!goalUi.goalBlocked || goalUi.mode) goalUi.toggle(); } },
    { label: "Details panel", icon: "radar", on: floatingOn, onClick: () => setTweak("layout", floatingOn ? "focus" : "rail") },
    "-",
    { label: "Command palette", icon: "command", hint: keyHint("desktop.commands.open"), onClick: () => setBridgeOpen(true) },
    { label: "Settings", icon: "cog", hint: keyHint("desktop.settings.open"), onClick: () => (stab.open ? closeSettings() : openSettings()) },
  ];

  // Chat links: project files to the preview, localhost to the browser panel.
  return (
    <_AL_MdLinkCtx.Provider value={handleChatLinkClick}>
      <div className="app-backdrop" />
      <div className="app">
        {/* `is-running`: the active tab streams; the always-visible loops
            (status/model dots, activity radar) run only then (styles.css). */}
        <div className={`window scanlines mode-${t.appMode === "chat" ? "chat" : "code"} ${showSplit ? "is-split" : ""}${streaming ? " is-running" : ""}`} style={panes.style}>
          <WindowChrome
            project={activeProject}
            profiles={profiles}
            activeProfileId={activeProfileId}
            startupProfileId={startupProfileId}
            onSelectProfile={handleSelectProfile}
            onCreateProfile={handleCreateProfile}
            onRenameProfile={handleRenameProfile}
            onDeleteProfile={handleDeleteProfile}
            onSetStartupProfile={handleSetStartupProfile}
            sidebarOpen={showSidebar}
            onToggleSidebar={toggleSidebar}
            canBack={!showHome}
            canForward={home && !noTab}
            onBack={() => setHome(true)}
            onForward={() => setHome(false)}
            mode={t.appMode}
            home={showHome}
            onMode={handleMode}
            toolbar={
              <ChromeToolbar
                onTerminal={() => terminal.toggle(planKey)}
                terminalOpen={_TERM_entryOf(terminal.state, planKey).open}
                onChanges={() => setGitOpen({ path: null })}
                moreItems={moreItems}
                chat={t.appMode === "chat"}
                onBrowser={() => toggleSide("browser")}
                browserOpen={side === "browser"}
                browserHint={keyHint("desktop.browser.toggle")}
                onSubagents={subagentUi.togglePane}
                subagentsOpen={showSplit}
                panelOpen={floatingOn}
                onPanel={() => setTweak("layout", floatingOn ? "focus" : "rail")}
              />
            }
          />
          <div ref={workspaceRef} className="workspace" style={panes.style}>
            {showSidebar && isChat && (
              <ChatSidebar
                tabs={chatTabs} saved={savedChats} activeId={showHome ? null : activeSessionId}
                pinned={pinned} archived={archived} unread={unread}
                onRename={handleRenameTab} onPin={handlePin} onArchive={handleArchiveRow} onDelete={handleDeleteRow} onMarkUnread={handleMarkUnread}
                onSelectTab={handleSelectTab} onCloseTab={handleCloseTab}
                onResume={handleResumeSession}
                onNew={() => setHome(true)} onHide={toggleSidebar}
                footer={<SidebarAccount avatar={t.accountAvatar} name={t.accountCall || t.accountName} plan={window.OMP_USAGE.quotaFor(quotas?.rows, model?.provider).mine?.plan} onClick={() => openSettings("account")} />}
              />
            )}
            {showSidebar && !isChat && (
              <ProjectSidebar
                tabs={codeTabs}
                unread={unread} onReveal={handleReveal} onMarkUnread={handleMarkUnread}
                onArchiveTab={tab => handleCloseTab(tab.id)}
                onDeleteTab={tab => trashConversation(tab.id, tab.sessionFile)}
                activeId={activeSessionId}
                recents={recentRows(recentProjects, sessions, activeProfileId).filter(r => r.path !== chatDir)}
                profileLabel={profileLabel}
                onSelectTab={handleSelectTab}
                onCloseTab={handleCloseTab}
                onRenameTab={handleRenameTab}
                onNewInProject={handleNewInProject}
                onOpenRecent={handleOpenRecent}
                onForgetRecent={handleForgetRecent}
                onOpenFolder={handleNewProject}
                onHide={toggleSidebar}
                onHistory={() => setHistoryOpen(true)}
                onReorder={handleReorder}
                footer={<SidebarAccount avatar={t.accountAvatar} name={t.accountCall || t.accountName} plan={window.OMP_USAGE.quotaFor(quotas?.rows, model?.provider).mine?.plan} onClick={() => openSettings("account")} />}
              />
            )}
            <div ref={stageRef} style={stageStyle} className={`stage ${side === "browser" ? `with-browser${browserFull ? " browser-full" : ""}` : sideRail ? `with-${sideRail}` : ""}`}>
              <main className="session" onFocusCapture={prAutomation.onConversationFocus}>
                {sessionWallpaper && <div className="wallpaper" style={{ "--wp": `url("${new URL(sessionWallpaper, document.baseURI).href}")` }} aria-hidden="true" />}
                {showHome ? (
                  <EmptyWorkspace notes={workspaceNotes}
                    recents={welcomeProjects}
                    onPickFolder={() => bridge?.pickFolder()}
                    onStart={handleStartFromWelcome}
                    chat={isChat ? chatDir : ""}
                    bridge={bridge} approvalModes={approvalModes} profileId={activeProfileId}
                    wallpaper={null}
                    favorites={t.favoriteModels} onManageModels={() => openSettings("models")} />
                ) : (<>
                  <ChatView key={activeSessionId} messages={messages}
                    planMode={plan.mode}
                    annotations={plan.annotations}
                    onAnnotate={handleAnnotate}
                    onAskAnswer={handleAskAnswer}
                    onAskDialogAnswer={handleAskDialogAnswer}
                    onConfirmAsk={handleConfirmAsk}
                    onCancelAsk={handleCancelAsk}
                    onGrantApproval={handleGrantApproval}
                    onStopRetry={handleStopRetry}
                    hoveredMsgIdx={hoveredMsgIdx}
                    hasProjectPath={!!activeProject?.path}
                    onInspectSubagent={subagentUi.open}
                    onOpenFile={handleOpenFile}
                    onPreviewFile={handlePreviewFile}
                    projectRoot={gitProject}
                    foldModes={foldModes}
                    collapseAfter={t.collapseAfter}
                    collapseOverrides={tabCollapseOverrides}
                    onToggleCollapse={handleToggleCollapse}
                    followOutput={t.followOutput}
                    submitKey={t.submitKey}
                    wallpaper={null}
                    onRewind={streaming ? null : handleRewind}
                    streaming={streaming}
                  />
                  <Composer
                    sessionId={planKey}
                    sessionIds={sessionIds}
                    onSend={handleSend}
                    planMode={plan.mode}
                    onTogglePlan={togglePlanMode}
                    planBlocked={goalUi.planBlocked}
                    goalMode={goalUi.mode}
                    goalTint={goalUi.tint}
                    onToggleGoal={goalUi.toggle}
                    goalBlocked={goalUi.goalBlocked}
                    goalStrip={goalUi.strip}
                    onOpenCmd={() => openBridge("commands")}
                    onOpenModel={() => openBridge("models")}
                    favoriteModels={t.favoriteModels}
                    onPickModel={handlePickModel}
                    onManageModels={() => openSettings("models")}
                    profileId={activeProfileId}
                    approvalMode={approvalCell}
                    onApprovals={() => setRulesOpen(true)}
                    contextInfo={liveCtx}
                    usage={{ provider: model?.provider, quotas, stats: sessionStats }}
                    onToggleDetails={() => setTweak("layout", floatingOn ? "focus" : "rail")}
                    currentModel={model}
                    thinking={thinkingLevel}
                    onLoadThinkingLevels={() => bridge.thinkingLevels()}
                    onSetThinking={level => bridge.setThinkingLevel(level)}
                    isStreaming={streaming}
                    backgroundWork={activeProject.runState === "background"}
                    onAbort={handleAbort}
                    onApprove={handleApprovePlan}
                    annotationCount={Object.keys(plan.annotations).length}
                    microcopy={data.microcopy}
                    onPick={handleCommand}
                    onFollowUp={handleFollowUp}
                    draftInsert={draftInsert}
                    promptHistory={promptHistory}
                    promptInsert={promptInsert}
                    queue={queue}
                    queueSending={queueSending}
                    onRemoveQueued={handleRemoveQueued}
                    onPromoteQueued={handlePromoteQueued}
                    onSendQueuedNow={handleSendQueuedNow}
                    browserChips={browser.chips}
                    onRemoveBrowserChip={browser.removeChip}
                    onBrowserChipClick={handleBrowserChipClick}
                    submitKey={t.submitKey}
                    pasteCollapseLines={t.pasteCollapseLines}
                  />
                </>)}
              </main>

              {splitShown && <ResizeHandle className="at-split" style={{ gridColumn: splitCol }} {...panes.handle("split")} />}
              {sideRail && <ResizeHandle key={sideRail} className="at-side" style={{ gridColumn: 2 }} {...panes.handle(sideRail)} />}
              {side === "browser" ? (
                <BrowserDock ui={browser} stageRef={stageRef} room={panes.browserRoom} />
              ) : side === "tree" ? (
                <ConversationTree
                  model={convTree.model} currentFile={convTree.currentFile} error={convTree.error}
                  loading={convTree.loading} treeKey={convTree.key} now={convTree.now} since={convTree.since}
                  streaming={streaming}
                  onRefresh={convTree.refresh} onClose={() => setSide("rail")}
                  onBranch={convTree.branch} onFork={convTree.fork} onOpenFile={convTree.openFile}
                />
              ) : showRail && !showHome && (
                <AmbientRail
                  project={activeProject}
                  profileLabel={profileLabel(activeProject.profile)}
                  ctxBreakdown={ctxBreakdown}
                  sessionStats={sessionStats}
                  provider={model?.provider}
                  quotas={quotas}
                  subagents={subagentList}
                  subagentPaneOpen={showSplit}
                  onOpenSubagent={subagentUi.open}
                  onToggleSubagentPane={subagentUi.togglePane}
                  show={{ subagents: t.showSubagents ?? true }}
                >
                  {/* Hidden without the GitHub CLI; the hook keeps checking on
                      focus, so installing gh brings it back. */}
                  {(t.showChanges ?? false) && !prAutomation.ghMissing && <PrAutomationCard key={planKey} automation={prAutomation} bridge={bridge}
                    available={!!activeProject.path && !!bridge?.isConnected} />}
                  {widgets.fatal && <WidgetErrorCard title="widgets.json" error={widgets.fatal} />}
                  {widgets.configErrors.map((e, i) => <WidgetErrorCard key={i} title="widgets.json" error={e} />)}
                  {widgets.approvals.map(a => (
                    <WidgetBoundary key={`approve:${a.id}`} title={a.title}>
                      <WidgetApprovalCard title={a.title} command={a.command} changed={a.changed}
                        onApprove={() => widgets.approve(a.id)} />
                    </WidgetBoundary>
                  ))}
                  {widgets.cards.map(c => (
                    <WidgetBoundary key={c.id} title={c.title} resetKey={c.resetKey}>
                      <WidgetCard title={c.title} run={c.run} attention={c.attention} onAck={() => widgets.acknowledge(c.id)}
                        onRefresh={() => widgets.refresh(c.id)} onInsert={handleWidgetInsert} insertCfg={c.insert} />
                    </WidgetBoundary>
                  ))}
                  {extCards.filter(c => t[c.tweak] !== false).map(c => (
                    <WidgetBoundary key={c.id} title={c.title} resetKey={c.run}>
                      <WidgetCard title={c.title} run={c.run} source="extension" onInsert={handleWidgetInsert} />
                    </WidgetBoundary>
                  ))}
                </AmbientRail>
              )}
              {showSplit && (
                <SubagentPane
                  state={subagents}
                  filter={subagentUi.filter} onFilter={subagentUi.setFilter}
                  selected={subagentUi.selected} onSelect={subagentUi.select}
                  level={subagentUi.level}
                  transcript={subagentUi.selected ? subagentTranscripts[subagentUi.selected.id] : null}
                  onLoadTranscript={id => bridge?.loadSubagentTranscript(id)}
                  onSteer={(id, text) => bridge?.steerSubagent(id, text)}
                  onStop={id => bridge?.cancelSubagent(id)}
                  onClose={subagentUi.closePane}
                  onJumpToCall={subagentUi.jumpToCall}
                  submitKey={t.submitKey}
                />
              )}


              {termShown && <ResizeHandle className="at-term" style={{ gridColumn: termCol }} {...panes.handle("term")} />}
              <TerminalPanels sessions={sessions} activeId={showHome ? null : planKey} terminal={terminal} />
            </div>
          </div>
          {showSidebar && <ResizeHandle className="at-sidebar" {...panes.handle("sidebar")} />}
        </div>
      </div>

      {settingsCtx && (
        <div className="st-overlay" onMouseDown={e => { if (e.target === e.currentTarget) closeSettings(); }}>
          <div className="st-modal" role="dialog" aria-label="Settings">
            <div className="st-modal-head">
              <span>Settings</span>
              <button type="button" className="btn icon ghost" onClick={closeSettings} aria-label="Close settings" title="Close (Esc)">×</button>
            </div>
            <div className="st-modal-body">
              <SettingsPage ctx={settingsCtx} />
            </div>
          </div>
        </div>
      )}

      <CommandBridge
        open={bridgeOpen}
        initialView={bridgeView}
        onClose={() => setBridgeOpen(false)}
        onPick={handleCommand}
        onPickModel={handlePickModel}
        onPickLogin={handlePickLogin}
        onInsertDraft={handleInsertDraft}
        loginProviders={loginProviders}
        currentModelId={model.id}
        profileId={activeProfileId}
      />

      {planOpen && (
        <PlanKanban
          kanban={kanban}
          planMeta={planMeta}
          onClose={() => setPlanOpen(false)}
          onAbort={handleAbort}
        />
      )}

      {historyOpen && (
        <HistoryModal
          open={historyOpen}
          onClose={() => setHistoryOpen(false)}
          onResume={handleResumeSession}
          activeCwd={activeProject?.path}
        />
      )}

      {gitOpen && (
        // Remounted per project, so no view state carries across; an
        // edit-card path preselects only in the project it came from. The
        // branch and the finished-edit count drive its refetches
        // (app/use-git-browser.jsx).
        <GitBrowser key={gitProject ?? ""} bridge={bridge}
          initialPath={gitOpen.project === gitProject ? gitOpen.path : null}
          onClose={closeGit} branch={activeProject.branch ?? null} edits={gitEdits} />
      )}

      {preview && preview.key === planKey && (
        <FilePreview target={preview.target} onClose={closePreview} onOpenGit={previewToGit} />
      )}

      {rulesOpen && (
        <ApprovalRulesPanel
          onClose={() => setRulesOpen(false)}
          activeProject={activeProject.id ? activeProject : null}
          sessions={sessions}
          approvalModes={approvalModes}
        />
      )}

      {statsOpen && (
        // Keyed on the profile, not just mounted once: `refresh` only runs
        // on mount/refresh-click (deps [bridge]), so without this a tab
        // switch to a different profile while the panel is open would
        // update the header's profileLabel but leave the previous
        // profile's numbers showing underneath it.
        <UsageStatsPanel key={activeProfileId} onClose={() => setStatsOpen(false)} profileLabel={activeProfileLabel} />
      )}

      {promptHistoryOpen && (
        <PromptHistoryModal
          open={promptHistoryOpen}
          entries={promptHistory}
          onClose={() => setPromptHistoryOpen(false)}
          onPick={handlePickHistoryPrompt}
        />
      )}


      {closeConfirm && (
        <ConfirmCloseDialog confirm={closeConfirm} onAnswer={handleCloseAnswer} />
      )}
    </_AL_MdLinkCtx.Provider>
  );
}


ReactDOM.createRoot(document.getElementById("root")).render(<App />);
