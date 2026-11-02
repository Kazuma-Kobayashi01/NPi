/* design/settings/rows-conversations.jsx — Conversation history management row
   (window.OMP_SETTINGS_ROWS.conversations): full dashboard for searching, previewing,
   renaming, pinning, archiving, exporting and bulk deleting saved sessions across projects. */

(function () {
  const { SettingsRow: Row, SettingsButton: Button, Icon } = window;

  function formatRelativeTime(ts) {
    if (!ts) return "—";
    const date = new Date(ts);
    if (isNaN(date.getTime())) return ts;
    const now = new Date();
    const diffSec = Math.floor((now - date) / 1000);
    if (diffSec < 60) return "just now";
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
    if (diffSec < 604800) return `${Math.floor(diffSec / 86400)}d ago`;
    return date.toLocaleDateString();
  }

  function formatBytes(bytes) {
    if (!bytes || isNaN(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const val = (bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1);
    return `${val} ${units[i]}`;
  }

  function formatDateFull(ts) {
    if (!ts) return "—";
    const date = new Date(ts);
    return isNaN(date.getTime()) ? ts : date.toLocaleString();
  }

  function _SRCV_Conversations({ row, ctx, query: schemaQuery }) {
    const [sessions, setSessions] = React.useState([]);
    const [loading, setLoading] = React.useState(true);
    const [error, setError] = React.useState("");
    const [toast, setToast] = React.useState("");

    // Filters and search
    const [searchQuery, setSearchQuery] = React.useState(schemaQuery || "");
    const [projectFilter, setProjectFilter] = React.useState("all");
    const [statusTab, setStatusTab] = React.useState("all"); // 'all' | 'active' | 'pinned' | 'archived'
    const [sortBy, setSortBy] = React.useState("recent"); // 'recent' | 'created' | 'turns' | 'size'

    // Selection and batch actions
    const [selectedPaths, setSelectedPaths] = React.useState(new Set());
    const [askingBatchDelete, setAskingBatchDelete] = React.useState(false);

    // Card-level state
    const [editingPath, setEditingPath] = React.useState(null);
    const [editingTitle, setEditingTitle] = React.useState("");
    const [askingDelete, setAskingDelete] = React.useState(null);
    const [exportMenuPath, setExportMenuPath] = React.useState(null);

    // Preview drawer
    const [previewSession, setPreviewSession] = React.useState(null);
    const [previewData, setPreviewData] = React.useState(null);
    const [previewLoading, setPreviewLoading] = React.useState(false);
    const [previewError, setPreviewError] = React.useState(null);

    // Pagination
    const [currentPage, setCurrentPage] = React.useState(1);
    const pageSize = 25;

    const pinnedList = ctx.t.pinnedSessions ?? [];
    const archivedList = ctx.t.archivedSessions ?? [];
    const pinnedSet = React.useMemo(() => new Set(pinnedList), [pinnedList]);
    const archivedSet = React.useMemo(() => new Set(archivedList), [archivedList]);

    // Toast helper
    const toastTimerRef = React.useRef(null);
    const showToast = (msg) => {
      setToast(msg);
      clearTimeout(toastTimerRef.current);
      toastTimerRef.current = setTimeout(() => setToast(""), 3200);
    };


    // Close export dropdown when clicking outside
    React.useEffect(() => {
      if (!exportMenuPath) return;
      const onDocClick = (e) => {
        if (!e.target.closest(".st-conv-export-anchor")) {
          setExportMenuPath(null);
        }
      };
      window.addEventListener("pointerdown", onDocClick);
      return () => window.removeEventListener("pointerdown", onDocClick);
    }, [exportMenuPath]);

    // Synchronize global settings search query into conversation search
    React.useEffect(() => {
      if (schemaQuery !== undefined) {
        setSearchQuery(schemaQuery);
      }
    }, [schemaQuery]);

    const activeProfileId = ctx.profileId || ctx.startupProfileId || "default";

    // Load sessions from disk
    const loadSessions = React.useCallback(async () => {
      setLoading(true);
      setError("");
      try {
        const list = await ctx.bridge?.listSavedSessions?.();
        setSessions(Array.isArray(list) ? list : []);
      } catch (err) {
        setError(`Failed to list conversations: ${String(err)}`);
      } finally {
        setLoading(false);
      }
    }, [ctx.bridge]);

    React.useEffect(() => {
      let cancelled = false;
      setLoading(true);
      setError("");
      Promise.resolve(ctx.bridge?.listSavedSessions?.())
        .then(list => {
          if (!cancelled) {
            setSessions(Array.isArray(list) ? list : []);
            setLoading(false);
          }
        })
        .catch(err => {
          if (!cancelled) {
            setError(`Failed to list conversations: ${String(err)}`);
            setLoading(false);
          }
        });
      return () => {
        cancelled = true;
        clearTimeout(toastTimerRef.current);
      };
    }, [ctx.bridge, activeProfileId]);

    // Reset pagination when filter criteria change
    React.useEffect(() => {
      setCurrentPage(1);
    }, [searchQuery, projectFilter, statusTab, sortBy]);

    // Available projects from sessions
    const projectOptions = React.useMemo(() => {
      const counts = new Map();
      for (const s of sessions) {
        const name = s.project_name || (s.cwd ? s.cwd.split("/").pop() : "Unknown");
        counts.set(name, (counts.get(name) || 0) + 1);
      }
      return Array.from(counts.entries())
        .sort((a, b) => a[0].localeCompare(b[0]))
        .map(([name, count]) => ({ key: name, label: `${name} (${count})` }));
    }, [sessions]);

    // Stats calculations
    const stats = React.useMemo(() => {
      let turns = 0;
      let bytes = 0;
      for (const s of sessions) {
        turns += s.message_count || 0;
        bytes += s.size_bytes || 0;
      }
      const profileKey = sessions[0]?.profile || ctx.profileId || ctx.startupProfileId || "default";
      const activeProfileObj = ctx.profiles?.find(p => p.id === profileKey);
      const activeProfileName = activeProfileObj?.name || profileKey;
      return {
        total: sessions.length,
        turns,
        bytes,
        profile: activeProfileName,
      };
    }, [sessions, ctx.profiles, ctx.profileId, ctx.startupProfileId]);

    // Filter and sort sessions
    const filteredSessions = React.useMemo(() => {
      let res = sessions;

      // Status filter
      if (statusTab === "active") {
        res = res.filter(s => !archivedSet.has(s.path));
      } else if (statusTab === "pinned") {
        res = res.filter(s => pinnedSet.has(s.path));
      } else if (statusTab === "archived") {
        res = res.filter(s => archivedSet.has(s.path));
      }

      // Project filter
      if (projectFilter !== "all") {
        res = res.filter(s => (s.project_name === projectFilter) || (s.cwd && s.cwd.split("/").pop() === projectFilter));
      }

      // Keyword query
      const q = searchQuery.toLowerCase().trim();
      if (q) {
        res = res.filter(s =>
          (s.title && s.title.toLowerCase().includes(q)) ||
          (s.project_name && s.project_name.toLowerCase().includes(q)) ||
          (s.cwd && s.cwd.toLowerCase().includes(q)) ||
          (s.preview && s.preview.toLowerCase().includes(q))
        );
      }

      // Sorting
      const copy = [...res];
      copy.sort((a, b) => {
        // If not sorting by a specific metric and not in archived tab, pinned items stay top
        if (statusTab !== "archived" && sortBy === "recent") {
          const aPinned = pinnedSet.has(a.path);
          const bPinned = pinnedSet.has(b.path);
          if (aPinned && !bPinned) return -1;
          if (!aPinned && bPinned) return 1;
        }

        if (sortBy === "created") {
          return (b.timestamp || "").localeCompare(a.timestamp || "");
        }
        if (sortBy === "turns") {
          return (b.message_count || 0) - (a.message_count || 0);
        }
        if (sortBy === "size") {
          return (b.size_bytes || 0) - (a.size_bytes || 0);
        }
        // default recent
        const aTime = a.updated_at || a.timestamp || "";
        const bTime = b.updated_at || b.timestamp || "";
        return bTime.localeCompare(aTime);
      });

      return copy;
    }, [sessions, statusTab, projectFilter, searchQuery, sortBy, pinnedSet, archivedSet]);

    // Pagination slice
    const totalPages = Math.max(1, Math.ceil(filteredSessions.length / pageSize));
    const pagedSessions = React.useMemo(() => {
      const start = (currentPage - 1) * pageSize;
      return filteredSessions.slice(start, start + pageSize);
    }, [filteredSessions, currentPage, pageSize]);

    // Master selection handlers
    const isAllPagedSelected = pagedSessions.length > 0 && pagedSessions.every(s => selectedPaths.has(s.path));
    const toggleSelectAllPaged = () => {
      const next = new Set(selectedPaths);
      if (isAllPagedSelected) {
        pagedSessions.forEach(s => next.delete(s.path));
      } else {
        pagedSessions.forEach(s => next.add(s.path));
      }
      setSelectedPaths(next);
      setAskingBatchDelete(false);
    };

    const toggleSelectSession = (path) => {
      const next = new Set(selectedPaths);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      setSelectedPaths(next);
      setAskingBatchDelete(false);
    };

    // Card Action: Inline Rename
    const startRename = (session) => {
      setEditingPath(session.path);
      setEditingTitle(session.title || "");
    };

    const saveRename = async (session) => {
      const trimmed = editingTitle.trim();
      setEditingPath(null);
      if (!trimmed || trimmed === session.title) return;
      try {
        const res = await ctx.bridge?.renameSavedSession?.(session.path, trimmed, session.profile);
        if (res && !res.ok) {
          setError(`Rename failed: ${res.error}`);
          return;
        }
        setSessions(prev => prev.map(s => s.path === session.path ? { ...s, title: trimmed } : s));
        if (previewSession?.path === session.path) {
          setPreviewSession(p => p ? { ...p, title: trimmed } : null);
        }
        showToast("Conversation renamed");
      } catch (err) {
        setError(`Rename error: ${String(err)}`);
      }
    };

    // Card Action: Pin/Unpin
    const togglePin = (path) => {
      const next = pinnedSet.has(path)
        ? pinnedList.filter(p => p !== path)
        : [...pinnedList, path];
      ctx.setTweak("pinnedSessions", next);
      showToast(pinnedSet.has(path) ? "Unpinned conversation" : "Pinned conversation");
    };

    // Card Action: Archive/Restore
    const toggleArchive = (path) => {
      const next = archivedSet.has(path)
        ? archivedList.filter(p => p !== path)
        : [...archivedList, path];
      ctx.setTweak("archivedSessions", next);
      showToast(archivedSet.has(path) ? "Restored conversation" : "Archived conversation");
    };

    // Card Action: Delete single
    const handleDeleteSingle = async (session) => {
      if (askingDelete !== session.path) {
        setAskingDelete(session.path);
        return;
      }
      setAskingDelete(null);
      setError("");
      try {
        const res = await ctx.bridge?.deleteSavedSession?.(session.path, session.profile);
        if (res && !res.ok) {
          setError(`Delete failed: ${res.error || "unknown error"}`);
          return;
        }
        setSessions(prev => prev.filter(s => s.path !== session.path));
        // clean tweaks
        if (pinnedSet.has(session.path)) {
          ctx.setTweak("pinnedSessions", pinnedList.filter(p => p !== session.path));
        }
        if (archivedSet.has(session.path)) {
          ctx.setTweak("archivedSessions", archivedList.filter(p => p !== session.path));
        }
        if (selectedPaths.has(session.path)) {
          const next = new Set(selectedPaths);
          next.delete(session.path);
          setSelectedPaths(next);
        }
        if (previewSession?.path === session.path) {
          setPreviewSession(null);
        }
        showToast("Conversation permanently deleted");
      } catch (err) {
        setError(`Delete error: ${String(err)}`);
      }
    };

    // Batch Actions
    const handleBatchArchive = (archive = true) => {
      const paths = Array.from(selectedPaths);
      const set = new Set(archivedList);
      paths.forEach(p => archive ? set.add(p) : set.delete(p));
      ctx.setTweak("archivedSessions", Array.from(set));
      setSelectedPaths(new Set());
      showToast(archive ? `Archived ${paths.length} conversations` : `Restored ${paths.length} conversations`);
    };

    const handleBatchDelete = async () => {
      if (!askingBatchDelete) {
        setAskingBatchDelete(true);
        return;
      }
      setAskingBatchDelete(false);
      setError("");
      const paths = Array.from(selectedPaths);
      const selectedSessions = sessions.filter(s => selectedPaths.has(s.path));
      const profile = selectedSessions[0]?.profile || ctx.startupProfileId || "default";
      try {
        const res = await ctx.bridge?.batchDeleteSavedSessions?.(paths, profile);
        if (res && !res.ok) {
          setError(`Batch delete failed: ${res.error || "unknown error"}`);
          return;
        }
        const deleted = new Set(Array.isArray(res?.value) ? res.value : paths);
        setSessions(prev => prev.filter(s => !deleted.has(s.path)));
        ctx.setTweak("pinnedSessions", pinnedList.filter(p => !deleted.has(p)));
        ctx.setTweak("archivedSessions", archivedList.filter(p => !deleted.has(p)));
        const remaining = new Set(Array.from(selectedPaths).filter(p => !deleted.has(p)));
        setSelectedPaths(remaining);
        if (previewSession && deleted.has(previewSession.path)) {
          setPreviewSession(null);
        }
        if (deleted.size < paths.length) {
          setError(`Deleted ${deleted.size} of ${paths.length} conversations. Active or protected sessions were kept.`);
        } else {
          showToast(`Deleted ${deleted.size} conversations`);
        }
      } catch (err) {
        setError(`Batch delete error: ${String(err)}`);
      }
    };

    // Export single session as Markdown
    const handleExportMarkdown = async (session) => {
      setExportMenuPath(null);
      const defaultOut = session.path.replace(/\.jsonl$/i, "") + ".md";
      try {
        const res = await ctx.bridge?.exportSessionMarkdown?.(session.path, defaultOut, session.profile);
        if (res && !res.ok) {
          setError(`Export failed: ${res.error}`);
          return;
        }
        await ctx.bridge?.revealPath?.(defaultOut);
        showToast(`Exported Markdown to ${defaultOut.split("/").pop()}`);
      } catch (err) {
        setError(`Export error: ${String(err)}`);
      }
    };

    // Batch Export Markdown
    const handleBatchExport = async () => {
      const paths = Array.from(selectedPaths);
      const byPath = new Map(sessions.map(s => [s.path, s]));
      let successCount = 0;
      for (const p of paths) {
        const outPath = p.replace(/\.jsonl$/i, "") + ".md";
        const profile = byPath.get(p)?.profile;
        const res = await ctx.bridge?.exportSessionMarkdown?.(p, outPath, profile);
        if (res?.ok) successCount++;
      }
      setSelectedPaths(new Set());
      showToast(`Batch exported ${successCount} markdown files`);
    };

    // Reveal in Finder
    const handleReveal = (session) => {
      setExportMenuPath(null);
      ctx.bridge?.revealPath?.(session.path);
    };

    // Resume / Open session
    const handleResume = async (session) => {
      if (!session || !session.path) return;
      setError("");
      try {
        await ctx.bridge?.resumeSession?.(session);
        ctx.close?.();
      } catch (err) {
        setError(`Failed to resume conversation: ${String(err?.message || err)}`);
      }
    };

    // Open Preview Drawer
    const handleOpenPreview = async (session) => {
      setPreviewSession(session);
      setPreviewLoading(true);
      setPreviewError(null);
      setPreviewData(null);
      try {
        const profile = session.profile || ctx.profileId || ctx.startupProfileId || "default";
        const res = await ctx.bridge?.conversationTree?.(session.path, profile);
        if (res && res.ok) {
          setPreviewData(res.value);
        } else {
          setPreviewError(res?.error || "Could not parse conversation tree");
        }
      } catch (err) {
        setPreviewError(String(err));
      } finally {
        setPreviewLoading(false);
      }
    };

    return (
      <Row stack label={row.label} desc={row.desc} query={schemaQuery} className="st-conversations-section">
        <div className="st-conv-wrap">
          {/* Toast Notification */}
          {toast && <div className="st-conv-toast">{toast}</div>}
          {error && (
            <div className="st-conv-alert">
              <span>{error}</span>
              <button type="button" className="btn ghost icon small" onClick={() => setError("")}>×</button>
            </div>
          )}

          {/* 1. Top Statistics Dashboard */}
          <div className="st-conv-stats-grid">
            <div className="st-conv-stat-card">
              <div className="st-conv-stat-icon"><Icon name="clock" size={18} /></div>
              <div className="st-conv-stat-content">
                <span className="st-conv-stat-num">{stats.total}</span>
                <span className="st-conv-stat-lbl">Conversations</span>
              </div>
            </div>
            <div className="st-conv-stat-card">
              <div className="st-conv-stat-icon"><Icon name="thinking" size={18} /></div>
              <div className="st-conv-stat-content">
                <span className="st-conv-stat-num">{stats.turns}</span>
                <span className="st-conv-stat-lbl">Total Turns</span>
              </div>
            </div>
            <div className="st-conv-stat-card">
              <div className="st-conv-stat-icon"><Icon name="file" size={18} /></div>
              <div className="st-conv-stat-content">
                <span className="st-conv-stat-num">{formatBytes(stats.bytes)}</span>
                <span className="st-conv-stat-lbl">Disk Usage</span>
              </div>
            </div>
            <div className="st-conv-stat-card">
              <div className="st-conv-stat-icon"><Icon name="agent" size={18} /></div>
              <div className="st-conv-stat-content">
                <span className="st-conv-stat-num">{stats.profile}</span>
                <span className="st-conv-stat-lbl">Active Profile</span>
              </div>
            </div>
          </div>

          {/* 2. Search & Filter Toolbar */}
          <div className="st-conv-toolbar">
            <div className="st-conv-search-box">
              <span className="st-conv-search-icon" aria-hidden="true">
                <Icon name="search" size={14} />
              </span>
              <input
                type="text"
                className="st-conv-search-input"
                placeholder="Search conversations by title, project, keywords..."
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
              />
              {searchQuery && (
                <button type="button" className="st-conv-search-clear" onClick={() => setSearchQuery("")} title="Clear search">×</button>
              )}
            </div>

            <div className="st-conv-filter-group">
              {/* Project filter */}
              <select
                className="st-conv-select"
                value={projectFilter}
                onChange={e => setProjectFilter(e.target.value)}
                aria-label="Filter by project">
                <option value="all">All Projects ({sessions.length})</option>
                {projectOptions.map(p => (
                  <option key={p.key} value={p.key}>{p.label}</option>
                ))}
              </select>

              {/* Status capsule tabs */}
              <div className="st-conv-pills" role="tablist">
                <button
                  type="button"
                  role="tab"
                  className={`st-conv-pill ${statusTab === "all" ? "active" : ""}`}
                  onClick={() => setStatusTab("all")}>
                  All
                </button>
                <button
                  type="button"
                  role="tab"
                  className={`st-conv-pill ${statusTab === "active" ? "active" : ""}`}
                  onClick={() => setStatusTab("active")}>
                  Active
                </button>
                <button
                  type="button"
                  role="tab"
                  className={`st-conv-pill ${statusTab === "pinned" ? "active" : ""}`}
                  onClick={() => setStatusTab("pinned")}>
                  Pinned ({pinnedList.length})
                </button>
                <button
                  type="button"
                  role="tab"
                  className={`st-conv-pill ${statusTab === "archived" ? "active" : ""}`}
                  onClick={() => setStatusTab("archived")}>
                  Archived ({archivedList.length})
                </button>
              </div>

              {/* Sort dropdown */}
              <select
                className="st-conv-select sort"
                value={sortBy}
                onChange={e => setSortBy(e.target.value)}
                aria-label="Sort conversations">
                <option value="recent">Recently Active</option>
                <option value="created">Created Date</option>
                <option value="turns">Most Turns</option>
                <option value="size">Largest Size</option>
              </select>

              <button
                type="button"
                className="btn ghost icon st-conv-refresh-btn"
                onClick={loadSessions}
                disabled={loading}
                title="Reload conversations from disk">
                <Icon name="refresh" size={14} />
              </button>
            </div>
          </div>

          {/* 3. List Header & Master Checkbox */}
          <div className="st-conv-list-header">
            <label className="st-conv-checkbox-label">
              <input
                type="checkbox"
                className="st-conv-checkbox"
                checked={isAllPagedSelected}
                onChange={toggleSelectAllPaged}
              />
              <span className="st-conv-header-count">
                {selectedPaths.size > 0
                  ? `${selectedPaths.size} selected`
                  : `Showing ${filteredSessions.length > 0 ? (currentPage - 1) * pageSize + 1 : 0}–${Math.min(currentPage * pageSize, filteredSessions.length)} of ${filteredSessions.length}`}
              </span>
            </label>

            {totalPages > 1 && (
              <div className="st-conv-mini-pages">
                <span>Page {currentPage} of {totalPages}</span>
              </div>
            )}
          </div>

          {/* 4. Conversation Cards List */}
          <div className="st-conv-list">
            {loading && sessions.length === 0 && (
              <div className="st-conv-empty">
                <Icon name="clock" size={24} />
                <p>Scanning disk for saved conversations…</p>
              </div>
            )}

            {!loading && filteredSessions.length === 0 && (
              <div className="st-conv-empty">
                <Icon name="search" size={24} />
                <p>
                  {searchQuery || projectFilter !== "all" || statusTab !== "all"
                    ? "No conversations match your filter criteria."
                    : "No saved conversation history found on disk."}
                </p>
              </div>
            )}

            {pagedSessions.map(s => {
              const isSelected = selectedPaths.has(s.path);
              const isPinned = pinnedSet.has(s.path);
              const isArchived = archivedSet.has(s.path);
              const isEditing = editingPath === s.path;
              const projectName = s.project_name || (s.cwd ? s.cwd.split("/").pop() : "Workspace");
              const relTime = formatRelativeTime(s.updated_at || s.timestamp);
              const isAskingDel = askingDelete === s.path;
              const hasExportMenu = exportMenuPath === s.path;

              return (
                <div
                  key={s.path}
                  className={`st-conv-card ${isSelected ? "selected" : ""} ${isPinned ? "is-pinned" : ""} ${isArchived ? "is-archived" : ""}`}>
                  
                  {/* Select Checkbox */}
                  <div className="st-conv-card-left">
                    <input
                      type="checkbox"
                      className="st-conv-checkbox"
                      checked={isSelected}
                      onChange={() => toggleSelectSession(s.path)}
                      aria-label={`Select ${s.title}`}
                    />
                  </div>

                  {/* Main Content Body */}
                  <div className="st-conv-card-body">
                    {/* Top Row: Title + inline edit + status chips */}
                    <div className="st-conv-card-top">
                      {isEditing ? (
                        <div className="st-conv-rename-inline">
                          <input
                            type="text"
                            autoFocus
                            className="st-conv-rename-input"
                            value={editingTitle}
                            onChange={e => setEditingTitle(e.target.value)}
                            onKeyDown={e => {
                              if (e.key === "Enter") saveRename(s);
                              if (e.key === "Escape") setEditingPath(null);
                            }}
                          />
                          <Button onClick={() => saveRename(s)}>Save</Button>
                          <Button onClick={() => setEditingPath(null)}>Cancel</Button>
                        </div>
                      ) : (
                        <div className="st-conv-card-title-group">
                          <h4
                            className="st-conv-card-title"
                            onDoubleClick={() => startRename(s)}
                            title="Double-click to rename">
                            {s.title || s.path.split("/").pop()}
                          </h4>
                          <button
                            type="button"
                            className="btn ghost icon st-conv-card-edit-btn"
                            onClick={() => startRename(s)}
                            title="Rename conversation">
                            <Icon name="edit" size={12} />
                          </button>
                          {isPinned && <span className="st-conv-tag pinned"><Icon name="tag" size={10} /> Pinned</span>}
                          {isArchived && <span className="st-conv-tag archived">Archived</span>}
                        </div>
                      )}

                      {/* Metadata Badges */}
                      <div className="st-conv-card-badges">
                        <span className="st-conv-badge project" title={s.cwd || projectName}>
                          <Icon name="folder" size={11} /> {projectName}
                        </span>
                        <span className="st-conv-badge turns" title="Total conversation messages">
                          <Icon name="context" size={11} /> {s.message_count || 1} turns
                        </span>
                        <span className="st-conv-badge size" title="Size on disk">
                          <Icon name="file" size={11} /> {formatBytes(s.size_bytes)}
                        </span>
                        <span className="st-conv-badge time" title={`Last updated: ${formatDateFull(s.updated_at || s.timestamp)}`}>
                          <Icon name="clock" size={11} /> {relTime}
                        </span>
                      </div>
                    </div>

                    {/* Preview summary snippet */}
                    <p className="st-conv-card-preview">
                      {s.preview ? s.preview.slice(0, 140) + (s.preview.length > 140 ? "…" : "") : "No preview recorded for this conversation."}
                    </p>

                    {/* Card Actions Bottom Row */}
                    <div className="st-conv-card-actions">
                      <Button accent onClick={() => handleResume(s)} title="Resume conversation in current tab or new tab">
                        Resume / Open
                      </Button>
                      <Button onClick={() => handleOpenPreview(s)} title="Inspect conversation structure and turns">
                        Preview
                      </Button>
                      <Button onClick={() => togglePin(s.path)} title={isPinned ? "Unpin conversation" : "Pin conversation to top"}>
                        {isPinned ? "Unpin" : "Pin"}
                      </Button>
                      <Button onClick={() => toggleArchive(s.path)} title={isArchived ? "Restore conversation" : "Archive conversation"}>
                        {isArchived ? "Restore" : "Archive"}
                      </Button>

                      {/* Export Dropdown */}
                      <div className="st-conv-export-anchor">
                        <Button
                          onClick={() => setExportMenuPath(hasExportMenu ? null : s.path)}
                          title="Export or reveal options">
                          Export ▾
                        </Button>
                        {hasExportMenu && (
                          <div className="st-conv-menu st-menu">
                            <button
                              type="button"
                              className="st-conv-menu-item"
                              onClick={() => handleExportMarkdown(s)}>
                              <Icon name="file" size={13} />
                              <span>Export Markdown (.md)</span>
                            </button>
                            <button
                              type="button"
                              className="st-conv-menu-item"
                              onClick={() => handleReveal(s)}>
                              <Icon name="external" size={13} />
                              <span>Reveal in Finder</span>
                            </button>
                          </div>
                        )}
                      </div>

                      {/* Delete Button with 2-click Confirmation */}
                      <Button
                        accent={isAskingDel}
                        onClick={() => handleDeleteSingle(s)}
                        title={isAskingDel ? "Click again to permanently delete" : "Delete conversation"}>
                        {isAskingDel ? "Delete permanently?" : "Delete"}
                      </Button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>

          {/* 5. Pagination Controls */}
          {totalPages > 1 && (
            <div className="st-conv-pagination">
              <button
                type="button"
                className="btn small"
                disabled={currentPage <= 1}
                onClick={() => setCurrentPage(p => Math.max(1, p - 1))}>
                ‹ Previous
              </button>

              <div className="st-conv-page-pills">
                {Array.from({ length: totalPages }, (_, i) => i + 1)
                  .filter(p => p === 1 || p === totalPages || Math.abs(p - currentPage) <= 2)
                  .map((p, idx, arr) => {
                    const prev = arr[idx - 1];
                    const gap = prev && p - prev > 1;
                    return (
                      <React.Fragment key={p}>
                        {gap && <span className="st-conv-page-ellipsis">…</span>}
                        <button
                          type="button"
                          className={`st-conv-page-btn ${currentPage === p ? "active" : ""}`}
                          onClick={() => setCurrentPage(p)}>
                          {p}
                        </button>
                      </React.Fragment>
                    );
                  })}
              </div>

              <button
                type="button"
                className="btn small"
                disabled={currentPage >= totalPages}
                onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}>
                Next ›
              </button>
            </div>
          )}

          {/* 6. Floating Action Bar for Batch Operations */}
          {selectedPaths.size > 0 && (
            <div className="st-conv-floating-bar slide-up">
              <div className="st-conv-floating-info">
                <span className="st-conv-floating-badge">{selectedPaths.size}</span>
                <span>conversations selected</span>
              </div>
              <div className="st-conv-floating-actions">
                <Button onClick={() => handleBatchArchive(true)}>Batch Archive</Button>
                <Button onClick={() => handleBatchArchive(false)}>Batch Restore</Button>
                <Button onClick={handleBatchExport}>Batch Export (.md)</Button>
                <Button
                  accent={askingBatchDelete}
                  onClick={handleBatchDelete}>
                  {askingBatchDelete ? `Confirm Delete (${selectedPaths.size})?` : "Batch Delete"}
                </Button>
                <button
                  type="button"
                  className="btn ghost small"
                  onClick={() => { setSelectedPaths(new Set()); setAskingBatchDelete(false); }}>
                  Deselect All
                </button>
              </div>
            </div>
          )}

          {/* 7. Slide-out Preview Drawer */}
          {previewSession && (
            <div className="st-conv-drawer-backdrop" onClick={() => setPreviewSession(null)}>
              <div className="st-conv-drawer slide-left" onClick={e => e.stopPropagation()}>
                <div className="st-conv-drawer-head">
                  <div className="st-conv-drawer-title-block">
                    <span className="st-conv-drawer-tag">Conversation Preview</span>
                    <h3 className="st-conv-drawer-title">{previewSession.title}</h3>
                    <span className="st-conv-drawer-path">{previewSession.path}</span>
                  </div>
                  <button
                    type="button"
                    className="btn ghost icon st-conv-drawer-close"
                    onClick={() => setPreviewSession(null)}
                    title="Close preview">
                    ×
                  </button>
                </div>

                <div className="st-conv-drawer-body">
                  <div className="st-conv-drawer-meta-grid">
                    <div><strong>Project:</strong> {previewSession.project_name || "Workspace"}</div>
                    <div><strong>Turns:</strong> {previewSession.message_count || 1}</div>
                    <div><strong>Size:</strong> {formatBytes(previewSession.size_bytes)}</div>
                    <div><strong>Last Active:</strong> {formatDateFull(previewSession.updated_at || previewSession.timestamp)}</div>
                  </div>

                  {previewLoading && (
                    <div className="st-conv-drawer-loading">
                      <Icon name="refresh" size={16} />
                      <span>Loading conversation tree…</span>
                    </div>
                  )}

                  {previewError && (
                    <div className="st-conv-drawer-error">
                      <span>{previewError}</span>
                    </div>
                  )}

                  {previewData && (
                    <div className="st-conv-drawer-sections">
                      {/* First User Prompt */}
                      <div className="st-conv-drawer-section">
                        <h5>First User Prompt</h5>
                        <div className="st-conv-drawer-prompt">
                          {(previewData.entries || []).find(e => e.kind === "user" || (!e.agent && e.text))?.text || "No prompt record available"}
                        </div>
                      </div>

                      {/* Latest Assistant Reply */}
                      <div className="st-conv-drawer-section">
                        <h5>Latest Assistant Reply</h5>
                        <div className="st-conv-drawer-reply">
                          {(previewData.entries || []).slice().reverse().find(e => e.agent || e.kind === "turn")?.text || "No assistant response recorded"}
                        </div>
                      </div>

                      {/* Turn & Token Breakdown */}
                      <div className="st-conv-drawer-section">
                        <h5>Session Summary</h5>
                        <div className="st-conv-drawer-tree-summary">
                          <span>Total Recorded Entries: <strong>{(previewData.entries || []).length}</strong></span>
                          <span>Tree Files: <strong>{(previewData.files || []).length}</strong></span>
                        </div>
                      </div>
                    </div>
                  )}
                </div>

                <div className="st-conv-drawer-foot">
                  <Button accent onClick={() => { handleResume(previewSession); setPreviewSession(null); }}>
                    Open This Conversation
                  </Button>
                  <Button onClick={() => setPreviewSession(null)}>Close</Button>
                </div>
              </div>
            </div>
          )}
        </div>
      </Row>
    );
  }

  Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, {
    conversations: _SRCV_Conversations,
  });
})();
