//! MCP server and Skill management IPC commands for NPi Desktop Settings.

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerItem {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env_keys: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub path: String,
    pub enabled: bool,
}

fn user_mcp_config_paths(app: &AppHandle) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(home) = app.path().home_dir() {
        paths.push(home.join(".local/share/npi/agent/mcp.json"));
        paths.push(home.join(".omp/agent/mcp.json"));
    }
    paths
}

fn read_user_mcp_json(app: &AppHandle) -> (PathBuf, Value) {
    let targets = user_mcp_config_paths(app);
    let primary = targets.first().cloned().unwrap_or_else(|| PathBuf::from("mcp.json"));

    for path in &targets {
        if path.is_file() {
            if let Ok(bytes) = fs::read(path) {
                if let Ok(val) = serde_json::from_slice::<Value>(&bytes) {
                    return (path.clone(), val);
                }
            }
        }
    }

    (primary, json!({
        "$schema": "https://raw.githubusercontent.com/can1357/oh-my-pi/main/packages/coding-agent/src/config/mcp-schema.json",
        "mcpServers": {}
    }))
}

fn save_user_mcp_json(app: &AppHandle, val: &Value) -> Result<(), String> {
    let targets = user_mcp_config_paths(app);
    let bytes = serde_json::to_vec_pretty(val).map_err(|e| e.to_string())?;

    for path in targets {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, &bytes);
    }
    Ok(())
}

#[tauri::command]
pub fn mcp_servers_get(app: AppHandle) -> Result<Vec<McpServerItem>, String> {
    let (_, val) = read_user_mcp_json(&app);
    let mut items = Vec::new();

    let disabled_set: Vec<String> = val.get("disabledServers")
        .and_then(|d| d.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    if let Some(servers) = val.get("mcpServers").and_then(|s| s.as_object()) {
        for (name, cfg) in servers {
            let command = cfg.get("command").and_then(|c| c.as_str()).map(String::from);
            let args = cfg.get("args")
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let env_keys = cfg.get("env")
                .and_then(|e| e.as_object())
                .map(|obj| obj.keys().cloned().collect())
                .unwrap_or_default();

            let transport = if cfg.get("url").is_some() { "http/sse" } else { "stdio" }.to_string();
            let explicit_enabled = cfg.get("enabled").and_then(|b| b.as_bool()).unwrap_or(true);
            let enabled = explicit_enabled && !disabled_set.contains(name);

            items.push(McpServerItem {
                name: name.clone(),
                transport,
                command,
                args,
                env_keys,
                enabled,
            });
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

#[tauri::command]
pub fn mcp_server_toggle(app: AppHandle, name: String, enabled: bool) -> Result<(), String> {
    let (_, mut val) = read_user_mcp_json(&app);

    if let Some(servers) = val.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
        if let Some(server) = servers.get_mut(&name).and_then(|s| s.as_object_mut()) {
            server.insert("enabled".to_string(), json!(enabled));
        }
    }

    let mut disabled = val.get("disabledServers")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    if enabled {
        disabled.retain(|v| v.as_str() != Some(&name));
    } else if !disabled.iter().any(|v| v.as_str() == Some(&name)) {
        disabled.push(json!(name));
    }

    if let Some(obj) = val.as_object_mut() {
        if disabled.is_empty() {
            obj.remove("disabledServers");
        } else {
            obj.insert("disabledServers".to_string(), Value::Array(disabled));
        }
    }

    save_user_mcp_json(&app, &val)
}

#[tauri::command]
pub fn mcp_server_delete(app: AppHandle, name: String) -> Result<(), String> {
    let (_, mut val) = read_user_mcp_json(&app);

    if let Some(servers) = val.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
        servers.remove(&name);
    }

    if let Some(disabled) = val.get_mut("disabledServers").and_then(|d| d.as_array_mut()) {
        disabled.retain(|v| v.as_str() != Some(&name));
    }

    save_user_mcp_json(&app, &val)
}

#[tauri::command]
pub fn mcp_server_upsert(
    app: AppHandle,
    name: String,
    command: String,
    args: Vec<String>,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Server name cannot be empty".to_string());
    }
    if command.trim().is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    let (_, mut val) = read_user_mcp_json(&app);
    let mut server_obj = serde_json::Map::new();
    server_obj.insert("type".to_string(), json!("stdio"));
    server_obj.insert("command".to_string(), json!(command.trim()));
    if !args.is_empty() {
        server_obj.insert("args".to_string(), json!(args));
    }

    if let Some(servers) = val.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
        servers.insert(name.trim().to_string(), Value::Object(server_obj));
    } else if let Some(root) = val.as_object_mut() {
        let mut map = serde_json::Map::new();
        map.insert(name.trim().to_string(), Value::Object(server_obj));
        root.insert("mcpServers".to_string(), Value::Object(map));
    }

    save_user_mcp_json(&app, &val)
}

fn parse_skill_file(path: &Path, source: &str) -> Option<SkillItem> {
    let text = fs::read_to_string(path).ok()?;
    let id = path.parent()?.file_name()?.to_string_lossy().to_string();

    let mut name = id.clone();
    let mut description = String::new();

    if let Some(front) = text.strip_prefix("---\n") {
        if let Some((yaml, _)) = front.split_once("\n---\n") {
            for line in yaml.lines() {
                if let Some(val) = line.strip_prefix("name:") {
                    name = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = line.strip_prefix("description:") {
                    description = val.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
    }

    if description.is_empty() {
        description = "User skill procedure".to_string();
    }

    Some(SkillItem {
        id,
        name,
        description,
        source: source.to_string(),
        path: path.to_string_lossy().to_string(),
        enabled: true,
    })
}

fn scan_skills_dir(dir: &Path, source: &str, items: &mut Vec<SkillItem>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let skill_file = p.join("SKILL.md");
                if skill_file.is_file() {
                    if let Some(item) = parse_skill_file(&skill_file, source) {
                        items.push(item);
                    }
                }
            }
        }
    }
}

#[tauri::command]
pub fn skills_list(app: AppHandle, project: Option<String>) -> Result<Vec<SkillItem>, String> {
    let mut items = Vec::new();
    // First, so a project skill that shares an id with a global one is the one kept.
    if let Some(dir) = project.as_deref().and_then(|p| project_skills_dir(p).ok()) {
        scan_skills_dir(&dir, "project", &mut items);
    }
    if let Ok(home) = app.path().home_dir() {
        scan_skills_dir(&home.join(".omp/agent/skills"), "user", &mut items);
        scan_skills_dir(&home.join(".local/share/npi/agent/skills"), "npi", &mut items);
        scan_skills_dir(&managed_skills_dir(&home), "managed", &mut items);
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items.dedup_by(|a, b| a.id == b.id);
    Ok(items)
}

/// Where the agent writes skills from what worked (omp's autolearn, `managed-skills/` beside the
/// built-in profile's agent folder). User-authored skills are never in it.
fn managed_skills_dir(home: &Path) -> PathBuf {
    crate::profiles::agent_dir_for(home, None, std::env::var_os("PI_CODING_AGENT_DIR").as_deref()).join("managed-skills")
}

/// Where the agent writes a project's skills: `<project>/.npi/skills`. `project` arrives from the
/// webview, so only an absolute path of an existing directory is accepted.
fn project_skills_dir(project: &str) -> Result<PathBuf, String> {
    let root = Path::new(project);
    if !root.is_absolute() || !root.is_dir() {
        return Err(format!("not a project folder: {project}"));
    }
    Ok(root.join(".npi").join("skills"))
}

/// The folder a managed skill is deleted from: the project's when `project` is given, else the
/// global `global` one.
fn managed_dir_for(global: PathBuf, project: Option<&str>) -> Result<PathBuf, String> {
    project.map_or(Ok(global), project_skills_dir)
}

/// Moves the managed skill `name` out of `managed` into `trash` (kept, not erased) and returns
/// where it went. Only a plain skill name inside `managed` is accepted.
fn set_aside(managed: &Path, trash: &Path, name: &str) -> Result<PathBuf, String> {
    let plain = !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-');
    if !plain {
        return Err(format!("not a managed skill name: {name}"));
    }
    let dir = managed.join(name);
    if !dir.join("SKILL.md").is_file() {
        return Err(format!("no managed skill named {name}"));
    }
    fs::create_dir_all(trash).map_err(|e| format!("skill trash: {e}"))?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let to = trash.join(format!("{name}-{stamp}"));
    fs::rename(&dir, &to).map_err(|e| format!("set aside {name}: {e}"))?;
    Ok(to)
}

/// Removes a skill the agent wrote: Undo on the notice, or Delete in Settings. It comes from
/// `<project>/.npi/skills` when `project` is given, else from the global managed folder. It is
/// moved to `~/.cache/npi/desktop/skill-trash/`, so a mistake can still be taken back by hand. A
/// running session keeps it listed until it next refreshes its skills.
#[tauri::command]
pub fn skill_delete_managed(app: AppHandle, name: String, project: Option<String>) -> Result<String, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    let trash = crate::npi_dirs::cache_dir()?.join("skill-trash");
    let managed = managed_dir_for(managed_skills_dir(&home), project.as_deref())?;
    set_aside(&managed, &trash, &name).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn skill_open(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(format!("Skill file does not exist: {path}"));
    }
    let mut cmd = std::process::Command::new("open");
    cmd.arg(p);
    crate::spawn_gate::status(&mut cmd)
        .map_err(|e| format!("open skill: {e}"))
        .and_then(|s| if s.success() { Ok(()) } else { Err("Failed to open skill in editor".to_string()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn a_managed_skill_is_set_aside_and_nothing_else_can_be() {
        let d = TempDir::new("mcp-skills", "set-aside");
        let managed = d.path().join("managed-skills");
        let trash = d.path().join("trash");
        fs::create_dir_all(managed.join("plan-trip")).unwrap();
        fs::write(managed.join("plan-trip/SKILL.md"), "---\nname: plan-trip\n---\nbody").unwrap();
        fs::write(d.path().join("SKILL.md"), "outside").unwrap();
        for bad in ["", "..", "../x", "Plan", "-x", "a/b"] {
            assert!(set_aside(&managed, &trash, bad).is_err(), "{bad:?}");
        }
        assert!(set_aside(&managed, &trash, "missing").is_err());
        let moved = set_aside(&managed, &trash, "plan-trip").unwrap();
        assert!(!managed.join("plan-trip").exists() && moved.join("SKILL.md").is_file());
        assert!(d.path().join("SKILL.md").is_file());
    }

    #[test]
    fn the_project_folder_is_resolved_only_for_an_existing_absolute_directory() {
        let d = TempDir::new("mcp-skills", "project-dir");
        let global = d.path().join("managed-skills");
        let project = d.path().to_string_lossy().into_owned();
        assert_eq!(managed_dir_for(global.clone(), None).unwrap(), global);
        assert_eq!(managed_dir_for(global.clone(), Some(&project)).unwrap(), d.path().join(".npi/skills"));
        for bad in ["", "relative/dir", ".", "~"] {
            assert!(managed_dir_for(global.clone(), Some(bad)).is_err(), "{bad:?}");
        }
        let missing = d.path().join("missing").to_string_lossy().into_owned();
        assert!(managed_dir_for(global.clone(), Some(&missing)).is_err());
        let file = d.path().join("file");
        fs::write(&file, "x").unwrap();
        let file = file.to_string_lossy().into_owned();
        assert!(managed_dir_for(global, Some(&file)).is_err());
    }

    #[test]
    fn a_project_skill_is_set_aside_and_its_sibling_is_left() {
        let d = TempDir::new("mcp-skills", "project-set-aside");
        let trash = d.path().join("trash");
        let project = d.path().join("proj");
        let skills = project.join(".npi/skills");
        for name in ["plan-trip", "keep-me"] {
            fs::create_dir_all(skills.join(name)).unwrap();
            fs::write(skills.join(name).join("SKILL.md"), "---\nname: x\n---\nbody").unwrap();
        }
        let project = project.to_string_lossy().into_owned();
        let dir = managed_dir_for(d.path().join("managed-skills"), Some(&project)).unwrap();
        let moved = set_aside(&dir, &trash, "plan-trip").unwrap();
        assert!(!skills.join("plan-trip").exists() && moved.join("SKILL.md").is_file());
        assert!(skills.join("keep-me/SKILL.md").is_file());
        assert!(set_aside(&dir, &trash, "../proj").is_err());
    }
}
