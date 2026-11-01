/* ═════════════════════════════════════════════════════════════════════
   design/settings/rows-mcp-skills.jsx — MCP Server & Skill Management
   Registered into window.OMP_SETTINGS_ROWS:
     mcpServers     lists/manages Model Context Protocol stdio/http servers
     skillsManager  lists/manages agent skills and opens in editor
   ═════════════════════════════════════════════════════════════════════ */

(function () {
  const { SettingsRow: Row, SettingsToggle: Toggle, SettingsButton: Button, Icon } = window;

  function _invokeMcp(cmd, args = {}) {
    if (!window.__TAURI__) return Promise.resolve({ ok: false, error: "Tauri unavailable" });
    return window.__TAURI__.core.invoke(cmd, args)
      .then(res => ({ ok: true, value: res }))
      .catch(err => ({ ok: false, error: String(err) }));
  }

  function McpServersRow({ row, query }) {
    const [servers, setServers] = React.useState([]);
    const [loading, setLoading] = React.useState(true);
    const [error, setError] = React.useState(null);
    const [adding, setAdding] = React.useState(false);
    const [newName, setNewName] = React.useState("");
    const [newCmd, setNewCmd] = React.useState("");
    const [newArgs, setNewArgs] = React.useState("");

    const loadServers = React.useCallback(() => {
      setLoading(true);
      setError(null);
      _invokeMcp("mcp_servers_get").then(res => {
        setLoading(false);
        if (res.ok) setServers(res.value || []);
        else setError(res.error);
      });
    }, []);

    React.useEffect(() => {
      loadServers();
    }, [loadServers]);

    const handleToggle = async (name, current) => {
      const next = !current;
      setServers(prev => prev.map(s => s.name === name ? { ...s, enabled: next } : s));
      const res = await _invokeMcp("mcp_server_toggle", { name, enabled: next });
      if (!res.ok) loadServers();
    };

    const handleDelete = async (name) => {
      setServers(prev => prev.filter(s => s.name !== name));
      await _invokeMcp("mcp_server_delete", { name });
    };

    const handleAdd = async (e) => {
      e.preventDefault();
      if (!newName.trim() || !newCmd.trim()) return;
      const parsedArgs = newArgs.trim() ? newArgs.trim().split(/\s+/) : [];
      const res = await _invokeMcp("mcp_server_upsert", {
        name: newName.trim(),
        command: newCmd.trim(),
        args: parsedArgs,
      });
      if (res.ok) {
        setNewName("");
        setNewCmd("");
        setNewArgs("");
        setAdding(false);
        loadServers();
      } else {
        setError(res.error);
      }
    };

    return (
      <Row stack label={row.label} desc={row.desc} query={query}>
        <div className="st-mcp-container" style={{ display: "flex", flexDirection: "column", gap: 10, width: "100%" }}>
          {error && <div className="st-row-error" style={{ color: "var(--rose)", fontSize: "var(--d-text-xs)" }}>{error}</div>}

          {loading && servers.length === 0 && (
            <div className="st-dim" style={{ fontSize: "var(--d-text-xs)" }}>Loading MCP servers…</div>
          )}

          {!loading && servers.length === 0 && !adding && (
            <div className="st-dim" style={{ fontSize: "var(--d-text-xs)" }}>No MCP servers configured in mcp.json.</div>
          )}

          {servers.map(s => (
            <div key={s.name} className="st-mcp-item" style={{
              display: "flex", alignItems: "center", justifyContent: "space-between",
              padding: "8px 12px", background: "var(--bg-elevated)", borderRadius: 8,
              border: "1px solid var(--line)"
            }}>
              <div style={{ display: "flex", flexDirection: "column", gap: 2, minWidth: 0, flex: 1, marginRight: 12 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  <span style={{ fontWeight: 600, color: "var(--fg)" }}>{s.name}</span>
                  <span className="chip muted mono" style={{ fontSize: 10, padding: "0 4px" }}>{s.transport}</span>
                </div>
                {s.command && (
                  <div className="mono" style={{ fontSize: "var(--d-text-xs)", color: "var(--fg-3)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {s.command} {s.args?.join(" ")}
                  </div>
                )}
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <Toggle value={s.enabled} label={`Toggle ${s.name}`} onChange={() => handleToggle(s.name, s.enabled)} />
                <button
                  type="button"
                  className="btn ghost icon"
                  style={{ color: "var(--fg-4)", height: 24, width: 24, padding: 0 }}
                  onClick={() => handleDelete(s.name)}
                  title={`Remove ${s.name}`}
                >
                  <Icon name="x" size={11} />
                </button>
              </div>
            </div>
          ))}

          {adding ? (
            <form onSubmit={handleAdd} style={{
              display: "flex", flexDirection: "column", gap: 8, padding: 10,
              background: "var(--bg-elevated)", borderRadius: 8, border: "1px dashed var(--accent)"
            }}>
              <div style={{ fontWeight: 600, fontSize: "var(--d-text-sm)", color: "var(--fg)" }}>Add Stdio Server</div>
              <input
                type="text"
                placeholder="Server name (e.g. github)"
                value={newName}
                onChange={e => setNewName(e.target.value)}
                style={{ background: "var(--bg-input)", border: "1px solid var(--line)", borderRadius: 4, padding: "4px 8px", color: "var(--fg)" }}
              />
              <input
                type="text"
                placeholder="Command (e.g. /path/to/binary or npx)"
                value={newCmd}
                onChange={e => setNewCmd(e.target.value)}
                style={{ background: "var(--bg-input)", border: "1px solid var(--line)", borderRadius: 4, padding: "4px 8px", color: "var(--fg)" }}
              />
              <input
                type="text"
                placeholder="Arguments (optional, space separated)"
                value={newArgs}
                onChange={e => setNewArgs(e.target.value)}
                style={{ background: "var(--bg-input)", border: "1px solid var(--line)", borderRadius: 4, padding: "4px 8px", color: "var(--fg)" }}
              />
              <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", marginTop: 4 }}>
                <Button onClick={() => setAdding(false)}>Cancel</Button>
                <Button accent type="submit">Save Server</Button>
              </div>
            </form>
          ) : (
            <div style={{ display: "flex", justifyContent: "flex-start", marginTop: 4 }}>
              <Button onClick={() => setAdding(true)}>+ Add MCP Server</Button>
            </div>
          )}
        </div>
      </Row>
    );
  }

  function SkillsManagerRow({ row, query }) {
    const [skills, setSkills] = React.useState([]);
    const [loading, setLoading] = React.useState(true);
    const [error, setError] = React.useState(null);

    const loadSkills = React.useCallback(() => {
      setLoading(true);
      setError(null);
      _invokeMcp("skills_list").then(res => {
        setLoading(false);
        if (res.ok) setSkills(res.value || []);
        else setError(res.error);
      });
    }, []);

    React.useEffect(() => {
      loadSkills();
    }, [loadSkills]);

    const handleOpen = (path) => {
      _invokeMcp("skill_open", { path });
    };

    return (
      <Row stack label={row.label} desc={row.desc} query={query}>
        <div style={{ display: "flex", flexDirection: "column", gap: 10, width: "100%" }}>
          {error && <div className="st-row-error" style={{ color: "var(--rose)", fontSize: "var(--d-text-xs)" }}>{error}</div>}

          {loading && skills.length === 0 && (
            <div className="st-dim" style={{ fontSize: "var(--d-text-xs)" }}>Scanning installed skills…</div>
          )}

          {!loading && skills.length === 0 && (
            <div className="st-dim" style={{ fontSize: "var(--d-text-xs)" }}>No skills found in ~/.omp/agent/skills.</div>
          )}

          {skills.map(s => (
            <div key={s.id} className="st-skill-item" style={{
              display: "flex", alignItems: "center", justifyContent: "space-between",
              padding: "8px 12px", background: "var(--bg-elevated)", borderRadius: 8,
              border: "1px solid var(--line)"
            }}>
              <div style={{ display: "flex", flexDirection: "column", gap: 2, minWidth: 0, flex: 1, marginRight: 12 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  <span style={{ fontWeight: 600, color: "var(--fg)" }}>{s.name}</span>
                  <span className="chip muted" style={{ fontSize: 10, padding: "0 4px" }}>{s.source}</span>
                </div>
                <div style={{ fontSize: "var(--d-text-xs)", color: "var(--fg-3)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={s.description}>
                  {s.description}
                </div>
              </div>
              <div>
                <Button onClick={() => handleOpen(s.path)} title="Open SKILL.md in external editor">
                  Open <Icon name="external" size={10} />
                </Button>
              </div>
            </div>
          ))}
        </div>
      </Row>
    );
  }

  window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {};
  Object.assign(window.OMP_SETTINGS_ROWS, {
    mcpServers: McpServersRow,
    skillsManager: SkillsManagerRow,
  });
})();
