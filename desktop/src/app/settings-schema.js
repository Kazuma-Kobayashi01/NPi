// Settings page (#37): sections, groups and rows, plus the pure helpers the
// page, its tab chip and the tests share.
//
// Exposes `window.OMP_SETTINGS`; wrapped as an IIFE per the project rule for
// plain <script> tags (see AGENTS.md "IIFE rule").
//
// Two row kinds:
//   - `tweak`: one key of TWEAK_DEFAULTS (app/constants.js), rendered by the
//     page itself; `type` is toggle | seg | slider | themes | swatches.
//   - `custom`: rendered by `window.OMP_SETTINGS_ROWS[type]`
//     (design/settings/rows-*.jsx). `keys` / `keyPrefix` name the tweak keys
//     the row owns, so its changes count in the badges and the section
//     reset; Rust-backed rows (shell, chromium, profiles, approvals,
//     keybindings) own none and never count.
// `desc` is schema-authored text; `backtick` segments render as code.
// Bounds of collapseAfter / promptHistoryLimit / pasteCollapseLines and the
// fold modes come from OMP_COLLAPSE / OMP_PROMPT_HISTORY / OMP_SESSION_UI,
// read late (getters), as are OMP_ACCENT (accent comparison), OMP_KEYMAP
// (shortcut keywords) and TWEAK_DEFAULTS, so load order among the plain
// app/*.js modules does not matter.
(function () {
  const THEMES = Object.freeze([
    { id: "graphite", label: "Graphite", tone: "dark", accent: "#3F8FEA" },
    { id: "ivory", label: "Ivory", tone: "light", accent: "#2B6FD6" },
    { id: "latte", label: "Latte", group: "Catppuccin", tone: "light", accent: "#8839EF" },
    { id: "frappe", label: "Frappé", group: "Catppuccin", tone: "dark", accent: "#CA9EE6" },
    { id: "macchiato", label: "Macchiato", group: "Catppuccin", tone: "dark", accent: "#C6A0F6" },
    { id: "mocha", label: "Mocha", group: "Catppuccin", tone: "dark", accent: "#CBA6F7" },
  ]);
  const ACCENTS = Object.freeze(["#3F8FEA", "#D97757", "#4FB37A", "#E8A33D", "#A58BF0", "#E26AA8"]);
  /** The accent a theme ships with; picking the theme resets to it. */
  const themeAccent = (theme) => (THEMES.find(t => t.id === theme) ?? THEMES[0]).accent;

  const defaults = () => window.TWEAK_DEFAULTS ?? {};
  const collapse = () => window.OMP_COLLAPSE;
  const history = () => window.OMP_PROMPT_HISTORY;
  const sessionUi = () => window.OMP_SESSION_UI;
  const opts = (values, labels = {}) => values.map(v => ({ value: v, label: labels[v] ?? String(v) }));
  const foldOptions = () => opts(collapse()?.MODES ?? ["auto", "open", "folded"]);

  // Descriptors, not spread: spread would read the bound getters once, now.
  const withExtra = (base, extra) => Object.defineProperties(base, Object.getOwnPropertyDescriptors(extra ?? {}));
  const tweak = (key, type, label, desc, kw, extra) => withExtra({ kind: "tweak", id: key, key, type, label, desc, kw }, extra);
  const custom = (type, label, desc, kw, extra) => withExtra({ kind: "custom", id: `custom:${type}`, type, label, desc, kw }, extra);

  const RAW_SECTIONS = [
    { id: "account", icon: "agent", title: "Account", blurb: "Who you are, for the Chat assistant to remember.", groups: [
      { name: "Profile", rows: [
        custom("account", "Profile", "Your photo and name show at the bottom of the sidebar. What you say here about yourself goes into every chat in Chat mode.", "account profile photo avatar name work interests hobby about memory",
          { keys: ["accountName", "accountCall", "accountAvatar"] }),
      ] },
    ] },
    { id: "appearance", icon: "sparkle", title: "Appearance", blurb: "Colour theme, accent, density and text size of the whole window.", groups: [
      { name: "Language", rows: [
        tweak("language", "seg", "Language", "The language of this interface. It does not change the language the model replies in or any system prompt: tell the model in your instructions or message.", "language locale i18n translation chinese japanese english 语言 中文 繁體 日本語",
          { stack: true, get options() { const I = window.OMP_I18N; return opts(["auto", ...(I?.LOCALES ?? [])], { auto: "Auto", ...(I?.NAMES ?? {}) }); } }),
      ] },
      { name: "Detail", rows: [
        custom("uiDetail", "Interface detail", "Five steps from the barest window to the fullest. A step sets the side panel, folding and the prompt box's extras at once; each can still be changed on its own.", "detail minimal simple full complexity density panel",
          { keys: ["uiDetail"] }),
      ] },
      { name: "Theme", rows: [
        tweak("theme", "themes", "Theme", "Picking a theme also resets the accent to the theme's own.", "dark light colour color graphite ivory catppuccin latte frappe macchiato mocha"),
        tweak("accent", "swatches", "Accent", "Focus rings, the active tab, links and the assistant's marks.", "color colour highlight"),
      ] },
      { name: "Wallpaper", rows: [
        tweak("wallpaper", "toggle", "Chat wallpaper", "A faint Renaissance artwork behind the conversation, drawn in the text colour. Off leaves the chat plain.", "wallpaper background art picture renaissance texture"),
        custom("wallpaperArt", "Artwork", "Which artwork the wallpaper shows.", "wallpaper artwork vitruvian athens venus adam melencolia last supper",
          { keys: ["wallpaperArt"], visibleIf: (v) => v?.wallpaper !== false }),
      ] },
      { name: "Layout", rows: [
        tweak("chatWidth", "seg", "Chat width", "How wide the conversation and the prompt box are.", "width narrow medium wide column reading",
          { options: opts(["narrow", "medium", "wide"]) }),
      ] },
      { name: "Text", rows: [
        tweak("density", "seg", "Density", "Row height, padding and the base text size.", "spacing compact",
          { options: opts(["cozy", "compact", "dense"]) }),
        tweak("fontSize", "slider", "Font size", "Scales all text and icons, the terminal's font included.", "zoom scale text",
          { min: 75, max: 150, step: 5, unit: "%" }),
        tweak("uiFont", "seg", "Font", "Typeface of the whole interface, serif or sans-serif. Code stays monospace.", "font serif sans typeface reading",
          { options: opts(["serif", "sans"], { serif: "Serif", sans: "Sans-serif" }) }),
        tweak("monoChat", "toggle", "Monospace chat", "Show messages in the monospace font instead of the UI font.", "font"),
      ] },
    ] },
    { id: "layout", icon: "grid", title: "Layout", blurb: "What sits beside the chat and how wide each column is.", groups: [
      { name: "Columns", rows: [
        tweak("layout", "seg", "Side column", "`rail` shows the widget rail, `split` opens a subagent beside the chat, `focus` shows the chat alone.", "layout rail split focus",
          { options: opts(["rail", "split", "focus"]) }),
        tweak("defaultSide", "seg", "New tabs show", "What the side column of a newly opened tab starts with. The toolbar still switches it per tab.", "conversation tree default browser",
          { options: opts(["rail", "tree", "browser"]) }),
        tweak("sidebar", "toggle", "Project sidebar", "The list of projects and their tabs on the left. `Ctrl+B` toggles it.", "projects"),
        custom("widths", "Column widths", "Drag a column edge to resize it, double-click the edge to reset one.", "width resize pane sidebar rail tree split browser",
          { keys: ["sidebarWidth", "railWidth", "treeWidth", "termWidth", "libraryWidth", "splitWidth", "browserWidth"] }),
      ] },
    ] },
    { id: "chat", icon: "thinking", title: "Chat", blurb: "How tool calls, workflow pipelines and thinking show up in the transcript.", groups: [
      { name: "Workflow pipeline", rows: [
        tweak("showWorkflowPipeline", "toggle", "Floating workflow pipeline", "Show a minimal horizontal stage capsule at the top of the transcript during planning and task execution.", "workflow pipeline stages flow kanban tasks"),
      ] },
      { name: "SideChat", rows: [
        custom("sidechatModel", "SideChat model", "Choose the model for independent, temporary conversations. Changes apply to the next message; an empty choice uses the chat worker's default model.", "sidechat model independent chat", { keys: ["sidechatModel"] }),
      ] },
      { name: "Chat assistant", rows: [
        custom("chatInstructions", "Instructions for Chat", "Standing instructions the assistant keeps in mind in every chat in Chat mode: tone, language, format. Applies to chats started or restarted after saving.", "instructions system prompt tone style persona"),
      ] },
      { name: "Folding", rows: [
        tweak("foldWork", "seg", "Thinking and tools", "`box` folds a turn's thinking and tool calls into one line you can open; `rows` shows each as its own row (the Detailed and Full steps).", "group box summary one line claude",
          { workMode: true, get options() { return opts(collapse()?.WORK_MODES ?? ["box", "rows"], { box: "one line", rows: "rows" }); } }),
        tweak("foldThinking", "seg", "Thinking", "`auto` folds it once newer turns arrive, `open` and `folded` fix it.", "collapse reasoning",
          { fold: "thinking", get options() { return foldOptions(); } }),
        tweak("foldTools", "seg", "Tool output", "Same choice for tool cards. A card you toggle yourself stays that way.", "collapse cards",
          { fold: "tool", get options() { return foldOptions(); } }),
        tweak("collapseAfter", "slider", "Collapse after", "With `auto`: how many newer turns before an entry folds.", "auto fold", {
          step: 1, unit: " newer",
          visibleIf: (values) => {
            const m = collapse()?.foldModes(values) ?? { thinking: values?.foldThinking ?? "auto", tool: values?.foldTools ?? "auto" };
            return m.thinking === "auto" || m.tool === "auto";
          },
          get min() { return collapse()?.MIN_AFTER ?? 1; },
          get max() { return collapse()?.MAX_AFTER ?? 30; },
        }),
      ] },
      { name: "Composer & sending", rows: [
        tweak("submitKey", "seg", "Send with", "In the composer, an ask's answer box and a subagent's steer box; the other chord inserts a newline. One-line fields always send with Enter.", "submit newline shift enter",
          { options: opts(["enter", "ctrl+enter"]) }),
        tweak("pasteCollapseLines", "slider", "Collapse pastes longer than", "A longer paste shows as a `[paste #1 +40 lines]` chip; the agent still gets the full text.", "paste chip",
          { step: 1, unit: " lines",
            get min() { return sessionUi()?.PASTE_LINES_MIN ?? 2; },
            get max() { return sessionUi()?.PASTE_LINES_MAX ?? 50; } }),
        tweak("promptHistoryLimit", "slider", "Prompt history", "How many of your prompts `↑` in the composer can recall, per tab.", "recall up arrow", {
          step: 10, unit: " prompts",
          get min() { return history()?.MIN_LIMIT ?? 10; },
          get max() { return history()?.MAX_LIMIT ?? 1000; },
        }),
        tweak("followOutput", "toggle", "Follow new output", "Keep the transcript scrolled to the newest message while the agent works. Scrolling up always pauses it.", "auto scroll bottom pin"),
      ] },
      { name: "Archive", rows: [
        custom("archive", "Archived conversations", "Conversations you archived from the sidebar. Restore brings one back to the list; Delete removes it from the disk and cannot be undone.", "archive archived restore delete conversations history",
          { keys: ["archivedSessions"] }),
      ] },
    ] },
    { id: "conversations", icon: "clock", title: "Conversations", blurb: "Manage, search, preview, export and organize your conversation history across projects.", groups: [
      { name: "Conversation history", rows: [
        custom("conversations", "Conversations", "Manage, search, preview, export and organize your conversation history across projects.", "conversations history search export archive preview sessions",
          { keys: ["pinnedSessions", "archivedSessions"] }),
      ] },
    ] },
    { id: "rail", icon: "radar", title: "Rail", blurb: "The cards in the rail beside the chat, including widget plugins and extension widgets.", groups: [
      { name: "Floating cards", rows: [
        tweak("showSubagents", "toggle", "Subagents", "Running and finished subagents of this tab.", "tasks"),
        tweak("showChanges", "toggle", "Changes", "Show the PR for this project's current branch. Needs the GitHub CLI (`gh`), signed in with `gh auth login`; the card stays hidden until `gh` is installed. Off also stops PR monitoring and turns off Auto-fix; enabling this card does not enable Auto-fix.", "github pull request pr auto fix ci checks review gh cli"),
      ] },
      { name: "Widgets", src: "widgets.json", rows: [
        custom("widgets", "Plugin config", "Plugins and their triggers. A changed script asks for approval again before it runs.", "widget plugins plugin widgets.json extension status reload",
          { keyPrefix: "widget:", prefixDefault: true }),
      ] },
    ] },
    { id: "models", icon: "bolt", title: "Models", blurb: "The default model, the ones the picker lists, and the vendors you can use.", groups: [
      { name: "Default", src: "omp config", rows: [
        custom("defaultModel", "Default model", "The model new conversations start on. It is omp's own default, so the terminal uses it too.", "model default start"),
      ] },
      { name: "Picker", rows: [
        custom("favoriteModels", "Favourite models", "The few models the small picker next to the prompt box lists. The others are under More models.", "favorite favourite common picker"),
      ] },
      { name: "Vendors", src: "omp login", rows: [
        custom("vendors", "Vendors", "Who the models come from. Signing in uses omp's own sign-in, so the terminal stays signed in too.", "vendor provider login sign in oauth anthropic openai google api"),
      ] },
    ] },
    { id: "memory", icon: "layers", title: "Memory", blurb: "What the assistant remembers about you, in Chat, Code and Academic.", groups: [
      { name: "Notes", src: "memory.db", rows: [
        custom("memory", "Memory", "The assistant notes short facts about you as you talk, such as your field, work, interests and projects, in Chat, Code and Academic, and reads them when a conversation starts. They stay on this computer. Off, nothing is noted or read.", "memory remember personal facts notes sqlite"),
      ] },
    ] },
    { id: "usage", icon: "cost", title: "Usage", blurb: "What is left of your subscriptions, and what this computer has used.", groups: [
      { name: "Subscription", src: "omp usage", rows: [
        custom("usageLimits", "Subscription limits", "How much of each limit is used, as each provider reports it.", "subscription quota limit remaining plan claude chatgpt codex antigravity"),
      ] },
      { name: "Statistics", src: "omp stats", rows: [
        custom("usageStats", "Usage statistics", "Requests, cost, tokens and speed from this computer's history, by model and folder.", "cost tokens stats usage spend"),
      ] },
    ] },
    { id: "terminal", icon: "terminal", title: "Terminal", blurb: "The shell panel under the chat. Each tab has its own shell, started in the tab's project folder.", groups: [
      { name: "Shell", src: "terminal.json", rows: [
        custom("shell", "Shell", "What new terminals run. Running ones keep their shell. `default` runs your default shell.", "powershell pwsh bash cmd"),
      ] },
      { name: "Panel", rows: [
        tweak("terminalAutoOpen", "toggle", "Open with new tab", "Show the terminal panel when a tab opens.", "auto"),
        tweak("terminalScrollback", "seg", "Scrollback", "Lines you can scroll back in each terminal. More costs memory per open shell.", "history lines buffer",
          { options: opts([1000, 5000, 10000, 50000]) }),
        custom("terminalHeight", "Height", "Shared by all tabs. Drag the panel's top edge to change it.", "size", { keys: ["terminalHeight"] }),
      ] },
    ] },
    { id: "browser", icon: "web", title: "Browser", blurb: "The Chromium panel the agent and you share in each tab.", groups: [
      { name: "Links", rows: [
        tweak("browserLocalhostInPanel", "toggle", "Open localhost links in the panel", "A click on a `localhost` link in the chat opens it here instead of your system browser.", "dev server url"),
      ] },
    ] },
    { id: "agent", icon: "agent", title: "Agent", blurb: "Agent profiles, engine execution behaviors, subagents, and extensions.", groups: [
      { name: "Profiles", src: "profiles.json", rows: [
        custom("startupProfile", "Startup profile", "The profile of the tab opened at launch and of new tabs.", "profile account"),
        custom("profileList", "Profiles", "A profile is a separate omp account space: its own login, conversations, settings and caches. Pick the one new tabs start in, switch the open tab to another (this restarts its agent), or add, rename and remove profiles. Removing only unlists it; the files stay on disk.", "profile profiles account login switch create rename delete multiple"),
      ] },
      { name: "Execution behaviors", rows: [
        tweak("interactionPolicy", "seg", "Interaction policy", "How readily the agent offers structured forms. Clear tasks run directly; permissions and required Ask dialogs stay separate.", "generative ui interaction form structured standard aggressive minimal choices parameters",
          { options: opts(["standard", "aggressive", "minimal"], { standard: "Standard", aggressive: "Aggressive", minimal: "Minimal" }) }),
        tweak("autoCompaction", "toggle", "Auto-compact context", "omp summarises older turns when the context fills up. Off: you run `/compact` yourself.", "compaction context window"),
        tweak("autoRetry", "toggle", "Retry failed requests", "omp retries a request the provider refused or dropped, with a retry row you can stop.", "retry error rate limit"),
        custom("goalContinue", "Keep working on a goal", "omp continues an active goal on its own after each turn instead of waiting for you. Costs tokens while unattended.", "goal auto continue"),
      ] },
      { name: "Subagents", rows: [
        custom("subagentModel", "Subagent default model", "Choose a model policy for delegated tasks: Ask before spawning, sync with the conversation, or use a Custom model and thinking level.", "subagent model thinking auto sync custom worker delegate", { keys: ["subagentModelMode", "subagentCustomModel", "subagentCustomThinking"] }),
        custom("subagentDelegation", "Subagent appetite", "Higher levels make the agent more inclined to delegate. These are concurrent upper limits, not target agent counts. Off disables delegation.", "delegation appetite subagent task parallel unlimited aggressive moderate conservative off", { keys: ["subagentDelegation"] }),
      ] },
      { name: "Model Context Protocol", src: "mcp.json", rows: [
        custom("mcpServers", "MCP Servers", "External tools and servers (stdio, HTTP/SSE) configured in mcp.json.", "mcp servers tools github model context protocol stdio http"),
      ] },
      { name: "Skills", src: "skills/", rows: [
        custom("skillsManager", "Agent Skills", "Installed skills, workflows and prompt procedures.", "skills custom prompt procedures workflow"),
      ] },
    ] },
    { id: "computerUse", icon: "web", title: "Computer Use", blurb: "Explicit control of screen capture, app interaction and native operation indicators.", groups: [
      { name: "Computer access", src: "omp config", rows: [
        custom("computerUse", "Computer Use", "Allow the agent to operate your Mac, only after you enable it and grant macOS permissions.", "computer use screen recording accessibility input monitoring permissions tcc desktop cursor stop control"),
      ] },
    ] },
    { id: "session", icon: "clock", title: "Session", blurb: "Tabs across restarts, closing, naming and app updates.", groups: [
      { name: "Tabs", rows: [
        tweak("restoreTabs", "toggle", "Reopen tabs on launch", "Start with the tabs and conversations that were open when you quit.", "restore session startup"),
        tweak("confirmBusyClose", "toggle", "Ask before closing a working tab", "Closing a tab whose agent is mid-turn stops it. Ask first, also when quitting.", "confirm quit close"),
        tweak("autoName", "toggle", "Name tabs automatically", "omp titles a new conversation from its first turns (one small model call). `/rename` always wins.", "rename title"),
      ] },
      { name: "Notifications", rows: [
        custom("notifications", "System notifications", "A banner from the system when the window is not in front: a long run finished, a question or approval is waiting, or a request failed. It names the project and says nothing of the conversation.", "notification notify banner alert toast permission sound",
          { keys: ["notifications", "notifyDone", "notifyAsk", "notifyFailed"] }),
      ] },
    ] },
    { id: "approvals", icon: "check", title: "Approvals", sep: true, blurb: "When the agent asks before it runs a tool.", groups: [
      { name: "Approval mode", src: "approval-mode.json", rows: [
        custom("approvalMode", "Tool approval mode", "When the agent asks before it runs a tool: a default for every project, and projects that override it. A change restarts omp in the affected open tabs; conversations continue.", "yolo write always ask permission project override default"),
      ] },
      { name: "Standing rules", rows: [
        custom("approvalRules", "Always-allow rules", "Rules are only granted from a prompt; here you review and revoke them.", "rules always allow revoke saved answers"),
      ] },
    ] },
    { id: "shortcuts", icon: "command", title: "Shortcuts", blurb: "Every app action and its chords. A change to one of omp's own bindings is saved as a desktop override.", groups: [
      { name: "Keyboard shortcuts", src: "keybindings.json", rows: [
        custom("shortcuts", "Key bindings", "Rebind an action, add a second chord or reset it to its default.", "keys hotkeys keybindings chord rebind shortcut",
          { moreKw: () => (window.OMP_KEYMAP?.KEYMAP_ACTIONS ?? []).map(a => `${a.label} ${a.id}`).join(" ") }),
      ] },
    ] },
  ];

  // Each settings destination gathers the relevant sections authored above.
  const MERGED = [
    { id: "account", icon: "agent", title: "Account", blurb: "Your profile and what the assistant remembers.", from: ["account"] },
    { id: "general", icon: "sparkle", title: "General", blurb: "Look, text, how tabs behave and the keyboard shortcuts.", from: ["appearance", "session", "shortcuts"] },
    { id: "chat", icon: "thinking", title: "Chat", blurb: "The transcript, workflow pipelines and the prompt box.", from: ["chat"] },
    { id: "conversations", icon: "clock", title: "Conversations", blurb: "Manage, search, preview, export and organize your conversation history across projects.", from: ["conversations"] },
    { id: "memory", icon: "layers", title: "Memory", blurb: "What the assistant remembers about you, in Chat, Code and Academic.", from: ["memory"] },
    { id: "models", icon: "bolt", title: "Models", blurb: "The default model, favourites and vendors.", from: ["models"] },
    { id: "usage", icon: "cost", title: "Usage", blurb: "What is left of your subscriptions, and what this computer has used.", from: ["usage"] },
    { id: "agent", icon: "agent", title: "Agent", blurb: "Profiles, execution behaviors, subagents, MCP and skills.", from: ["agent", "approvals"] },
    { id: "computerUse", icon: "web", title: "Computer Use", blurb: "Screen access, app control and macOS permissions.", from: ["computerUse"] },
    { id: "code", icon: "terminal", title: "Code", sep: true, blurb: "Side column and column widths, terminal, browser and the floating cards.", from: ["layout", "terminal", "browser", "rail"] },
  ];
  /** Authored sections no destination gathers: never shown. A test keeps this empty. */
  const UNPLACED = RAW_SECTIONS.filter(r => !MERGED.some(m => m.from.includes(r.id))).map(r => r.id);
  const SECTIONS = MERGED.map(m => ({
    id: m.id, icon: m.icon, title: m.title, sep: m.sep, blurb: m.blurb,
    groups: m.from.flatMap(id => RAW_SECTIONS.find(r => r.id === id).groups),
  }));

  const sectionById = (id) => SECTIONS.find(s => s.id === id);
  const sectionRows = (s) => s.groups.flatMap(g => g.rows);
  const ROWS = SECTIONS.flatMap(sectionRows);

  /** Every row, in page order. */
  const rows = () => ROWS;

  /** The value a tweak row's control shows: fold modes resolved, numbers clamped. */
  function value(row, values) {
    const v = values?.[row.key];
    if (row.fold) return collapse()?.foldModes(values)[row.fold] ?? v ?? "auto";
    if (row.workMode) return collapse()?.foldModes(values).work ?? v ?? "box";
    return clamp(row, v === undefined ? defaults()[row.key] : v);
  }

  /** `v` made valid for the row: sliders rounded to their step within bounds,
   *  segments one of their options, toggles a boolean; else the default. */
  function clamp(row, v) {
    const fallback = defaults()[row.key];
    if (row.type === "slider") {
      const n = Number(v);
      if (v === null || v === "" || !Number.isFinite(n)) return fallback;
      const stepped = row.min + Math.round((n - row.min) / row.step) * row.step;
      return Math.min(row.max, Math.max(row.min, stepped));
    }
    if (row.type === "seg") return row.options.some(o => o.value === v) ? v : fallback;
    if (row.type === "toggle") return typeof v === "boolean" ? v : fallback;
    return v ?? fallback;
  }

  /** Shown now: a row with `visibleIf` asks it (collapseAfter). */
  const visible = (row, values) => row.visibleIf?.(values) ?? true;

  // Tweak keys a custom row owns that differ from their defaults. A
  // `keyPrefix` row counts only `opts.prefixKeys` when given: the keys it
  // renders a control for (a stored key of a removed plugin has no row).
  function ownedChanged(row, values, opts) {
    const d = defaults();
    const changed = (row.keys ?? []).filter(k => k in (values ?? {}) && values[k] !== d[k]);
    if (row.keyPrefix) {
      for (const k of opts?.prefixKeys ?? Object.keys(values ?? {})) {
        if (k.startsWith(row.keyPrefix) && k in (values ?? {}) && values[k] !== row.prefixDefault) changed.push(k);
      }
    }
    return changed;
  }

  /** Changed from its default. Accent compares with the current theme's
   *  own accent (a theme pick resets it); fold modes compare resolved.
   *  `opts.prefixKeys`: see ownedChanged. */
  function isModified(row, values, opts) {
    if (row.kind === "custom") return ownedChanged(row, values, opts).length > 0;
    if (row.key === "accent") return !window.OMP_ACCENT.sameColor(values?.accent, themeAccent(values?.theme));
    return value(row, values) !== value(row, defaults());
  }

  /** Edits for setTweak that put one row back to its default. */
  function resetRow(row, values, opts) {
    const d = defaults();
    if (row.kind === "custom") {
      const edits = {};
      for (const k of ownedChanged(row, values, opts)) edits[k] = row.keyPrefix && k.startsWith(row.keyPrefix) ? row.prefixDefault : d[k];
      return edits;
    }
    if (row.key === "theme") return { theme: d.theme, accent: themeAccent(d.theme) };
    if (row.key === "accent") return { accent: themeAccent(values?.theme) };
    return { [row.key]: d[row.key] };
  }

  const scope = (sectionId) => (sectionId && sectionId !== "all" ? [sectionById(sectionId)].filter(Boolean) : SECTIONS);

  /** Changed visible rows of a section (or of all, for `"all"`/nothing). */
  const modifiedCount = (sectionId, values, opts) =>
    scope(sectionId).flatMap(sectionRows).filter(r => visible(r, values) && isModified(r, values, opts)).length;

  /** Edits for setTweak that reset every changed row of `rows`.
   *  Rows see the edits before them, so a theme reset settles the accent. */
  function resetRows(rows, values, opts) {
    const edits = {};
    for (const r of rows) {
      const next = { ...values, ...edits };
      if (isModified(r, next, opts)) Object.assign(edits, resetRow(r, next, opts));
    }
    return edits;
  }

  /** resetRows over a section (or all). */
  const resetEdits = (sectionId, values, opts) => resetRows(scope(sectionId).flatMap(sectionRows), values, opts);

  // `moreKw`: keywords known only at runtime (the shortcut action names).
  // Custom rows show `label` and `desc` too, plus runtime text of their own;
  // `version` shows runtime text as its desc, `shortcuts` only its editor.
  // The text in the UI language is searched as well as the English one.
  const tr = (s) => (s ? window.OMP_I18N?.t(s) ?? s : s);
  const searchText = (row) =>
    `${row.label} ${tr(row.label)} ${(row.desc ?? "").replace(/`/g, "")} ${tr(row.desc ?? "").replace(/`/g, "")} ${row.kw ?? ""} ${row.moreKw?.() ?? ""}`.toLowerCase();

  /** Visible rows whose label, desc or keywords contain the query,
   *  case-insensitive, grouped by section › group in page order. */
  function search(query, values) {
    const q = String(query ?? "").trim().toLowerCase();
    const bySection = Object.fromEntries(SECTIONS.map(s => [s.id, 0]));
    const groups = [];
    let total = 0;
    if (!q) return { total, bySection, groups };
    for (const s of SECTIONS) {
      for (const g of s.groups) {
        const hits = g.rows.filter(r => visible(r, values) && searchText(r).includes(q));
        if (!hits.length) continue;
        groups.push({ sectionId: s.id, sectionTitle: s.title, groupName: g.name, src: g.src, rows: hits });
        bySection[s.id] += hits.length;
        total += hits.length;
      }
    }
    return { total, bySection, groups };
  }

  window.OMP_SETTINGS = Object.freeze({
    SECTIONS, UNPLACED, THEMES, ACCENTS, themeAccent,
    rows, value, clamp, visible, isModified, resetRow, modifiedCount, resetRows, resetEdits, search,
  });
})();
