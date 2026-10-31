//! The Chat space: one folder every chat conversation runs in, the user's
//! instructions for the chat assistant and its memory.
//!
//! A chat is an ordinary omp session whose cwd is the chat folder, so chats
//! and code projects never mix (the sidebar tells them apart by that folder).
//! The instructions (`chat-instructions.md`, kept beside the app's other
//! config) and the memory (`MEMORY.md` inside the chat folder, so the
//! assistant can update it itself) are added to the Chat prompt at each omp
//! start (`system_prompt`).

use std::path::{Path, PathBuf};

const INSTRUCTIONS_FILE: &str = "chat-instructions.md";
const MEMORY_FILE: &str = "MEMORY.md";
const PROFILE_FILE: &str = "chat-profile.json";
/// A larger file is cut: the prompt must stay small.
const MAX_BYTES: usize = 16 * 1024;

/// What the user told the app about themselves (Settings → Account).
#[derive(Default, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    /// What the assistant should call them.
    pub call: String,
    pub work: String,
    pub interests: String,
    pub about: String,
}

pub struct ChatSpace {
    dir: PathBuf,
    config: PathBuf,
    /// Where chats lived before the workspace folder existed; still read as chats.
    legacy: Option<PathBuf>,
}

impl ChatSpace {
    /// `dir`: the chat folder; `config`: the folder for the instructions file.
    pub fn new(dir: PathBuf, config: PathBuf) -> Self {
        Self { dir, config, legacy: None }
    }

    /// Also treat `legacy` as a chat folder.
    pub fn with_legacy(mut self, legacy: PathBuf) -> Self {
        self.legacy = Some(legacy);
        self
    }

    /// Every folder whose conversations are chats: the current one first.
    pub fn dirs(&self) -> Vec<&Path> {
        let mut out = vec![self.dir.as_path()];
        if let Some(l) = self.legacy.as_deref().filter(|l| *l != self.dir && l.exists()) {
            out.push(l);
        }
        out
    }

    /// Creates the chat folder if needed.
    pub fn ensure_dir(&self) -> Result<&Path, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("chat folder: {e}"))?;
        Ok(&self.dir)
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path)
            .map(|s| crate::text::prefix_within(&s, MAX_BYTES).to_string())
            .unwrap_or_default()
    }

    pub fn instructions(&self) -> String {
        Self::read(&self.config.join(INSTRUCTIONS_FILE))
    }

    pub fn memory(&self) -> String {
        Self::read(&self.dir.join(MEMORY_FILE))
    }

    pub fn profile(&self) -> Profile {
        std::fs::read_to_string(self.config.join(PROFILE_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_profile(&self, p: &Profile) -> Result<(), String> {
        let cut = |s: &str| crate::text::prefix_within(s, 2048).to_string();
        let p = Profile { call: cut(&p.call), work: cut(&p.work), interests: cut(&p.interests), about: cut(&p.about) };
        std::fs::create_dir_all(&self.config).map_err(|e| format!("config folder: {e}"))?;
        let json = serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?;
        std::fs::write(self.config.join(PROFILE_FILE), json).map_err(|e| format!("profile: {e}"))
    }

    pub fn save(&self, instructions: &str, memory: &str) -> Result<(), String> {
        let cut = |s: &str| crate::text::prefix_within(s, MAX_BYTES).to_string();
        std::fs::create_dir_all(&self.config).map_err(|e| format!("config folder: {e}"))?;
        std::fs::write(self.config.join(INSTRUCTIONS_FILE), cut(instructions))
            .map_err(|e| format!("instructions: {e}"))?;
        self.ensure_dir()?;
        std::fs::write(self.dir.join(MEMORY_FILE), cut(memory)).map_err(|e| format!("memory: {e}"))
    }

    /// The Chat prompt: the base text, then the user's instructions and memory.
    pub fn system_prompt(&self, base: &str) -> String {
        let with_profile = compose_profile(base, &self.profile());
        compose(&with_profile, &self.instructions(), &self.memory(), &self.dir.join(MEMORY_FILE))
    }
}

fn compose_profile(base: &str, p: &Profile) -> String {
    let mut lines = Vec::new();
    if !p.call.trim().is_empty() {
        lines.push(format!("Call the user: {}", p.call.trim()));
    }
    if !p.work.trim().is_empty() {
        lines.push(format!("Their work: {}", p.work.trim()));
    }
    if !p.interests.trim().is_empty() {
        lines.push(format!("Their interests: {}", p.interests.trim()));
    }
    if !p.about.trim().is_empty() {
        lines.push(format!("About them: {}", p.about.trim()));
    }
    if lines.is_empty() {
        return base.to_string();
    }
    format!("{base}

What the user told you about themselves:
{}", lines.join("\n"))
}

fn compose(base: &str, instructions: &str, memory: &str, memory_path: &Path) -> String {
    let mut out = base.to_string();
    let instructions = instructions.trim();
    if !instructions.is_empty() {
        out.push_str("\n\nThe user's standing instructions for how to answer (follow them):\n");
        out.push_str(instructions);
    }
    out.push_str(&format!(
        "\n\nMemory: notes about the user that persist across chats live in {}. When the user asks \
         you to remember something, add it to that file (keep it short, one line per fact).",
        memory_path.display()
    ));
    let memory = memory.trim();
    if !memory.is_empty() {
        out.push_str("\nWhat you remember now:\n");
        out.push_str(memory);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space() -> (tempfile_dir::Dir, ChatSpace) {
        let d = tempfile_dir::Dir::new();
        let s = ChatSpace::new(d.path().join("Chats"), d.path().join("cfg"));
        (d, s)
    }

    /// A tiny temp dir (no dev-dependency needed).
    mod tempfile_dir {
        use std::path::{Path, PathBuf};
        pub struct Dir(PathBuf);
        impl Dir {
            pub fn new() -> Self {
                let p = std::env::temp_dir().join(format!("npi-chat-{}-{}", std::process::id(), rand_id()));
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn rand_id() -> u128 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        }
    }

    #[test]
    fn nothing_saved_reads_as_empty_and_the_prompt_still_names_the_memory_file() {
        let (_d, s) = space();
        assert_eq!(s.instructions(), "");
        assert_eq!(s.memory(), "");
        let p = s.system_prompt("BASE");
        assert!(p.starts_with("BASE"));
        assert!(p.contains("MEMORY.md"));
        assert!(!p.contains("standing instructions"));
    }

    #[test]
    fn saved_instructions_and_memory_reach_the_prompt() {
        let (_d, s) = space();
        s.save("Answer in Chinese.", "Likes tea.").unwrap();
        assert_eq!(s.instructions(), "Answer in Chinese.");
        assert_eq!(s.memory(), "Likes tea.");
        let p = s.system_prompt("BASE");
        assert!(p.contains("Answer in Chinese."));
        assert!(p.contains("Likes tea."));
    }

    #[test]
    fn the_profile_round_trips_and_reaches_the_prompt() {
        let (_d, s) = space();
        assert_eq!(s.profile(), Profile::default());
        let p = Profile { call: "Kazuma".into(), work: "Engineering".into(), interests: "tea".into(), about: String::new() };
        s.save_profile(&p).unwrap();
        assert_eq!(s.profile(), p);
        let text = s.system_prompt("BASE");
        assert!(text.contains("Call the user: Kazuma"));
        assert!(text.contains("Their work: Engineering"));
        assert!(!text.contains("About them"));
    }

    #[test]
    fn the_legacy_folder_counts_only_while_it_exists() {
        let (d, s) = space();
        let legacy = d.path().join("old");
        let s = s.with_legacy(legacy.clone());
        assert_eq!(s.dirs().len(), 1);
        std::fs::create_dir_all(&legacy).unwrap();
        assert_eq!(s.dirs().len(), 2);
        assert_eq!(s.dirs()[0], d.path().join("Chats"));
    }

    #[test]
    fn a_huge_text_is_cut() {
        let (_d, s) = space();
        s.save(&"x".repeat(MAX_BYTES * 2), "").unwrap();
        assert_eq!(s.instructions().len(), MAX_BYTES);
    }
}
