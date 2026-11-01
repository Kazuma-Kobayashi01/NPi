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
pub fn skills_list(app: AppHandle) -> Result<Vec<SkillItem>, String> {
    let mut items = Vec::new();
    if let Ok(home) = app.path().home_dir() {
        scan_skills_dir(&home.join(".omp/agent/skills"), "user", &mut items);
        scan_skills_dir(&home.join(".local/share/npi/agent/skills"), "npi", &mut items);
        scan_skills_dir(&home.join(".omp/agent/managed-skills"), "managed", &mut items);
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items.dedup_by(|a, b| a.id == b.id);
    Ok(items)
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
