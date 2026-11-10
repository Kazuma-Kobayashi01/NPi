//! Which subagent frames each window needs from a session's omp.
//!
//! omp keeps one subscription level per process, but two windows read the
//! same process: the main window (the subagent manager) and an Office
//! window (member conversations). Each claims the level it needs and omp is
//! told the highest, so one window lowering its own need never cuts the
//! other's frames off.

use std::collections::HashMap;
use std::sync::Mutex;

/// omp's subscription levels, lowest first.
const LEVELS: [&str; 3] = ["off", "progress", "events"];

fn rank(level: &str) -> Option<usize> {
    LEVELS.iter().position(|l| *l == level)
}

/// session id → window label → rank in [`LEVELS`].
#[derive(Default)]
pub struct SubscriptionClaims(Mutex<HashMap<String, HashMap<String, usize>>>);

impl SubscriptionClaims {
    /// Records that `owner` needs `level` of `session`; the level omp should run at.
    pub fn claim(&self, session: &str, owner: &str, level: &str) -> Result<&'static str, String> {
        let rank = rank(level).ok_or_else(|| format!("unknown subscription level: {level}"))?;
        let mut claims = self
            .0
            .lock()
            .map_err(|_| "subscription claims poisoned".to_string())?;
        let owners = claims.entry(session.to_string()).or_default();
        owners.insert(owner.to_string(), rank);
        let highest = owners.values().copied().max().unwrap_or(0);
        drop(claims);
        Ok(LEVELS[highest])
    }

    /// Drops every claim of `owner` (its window closed). Returns each session
    /// it had a claim on with the level omp should now run at.
    pub fn release(&self, owner: &str) -> Vec<(String, &'static str)> {
        let Ok(mut claims) = self.0.lock() else {
            return Vec::new();
        };
        let mut changed = Vec::new();
        claims.retain(|session, owners| {
            if owners.remove(owner).is_some() {
                changed.push((
                    session.clone(),
                    LEVELS[owners.values().copied().max().unwrap_or(0)],
                ));
            }
            !owners.is_empty()
        });
        changed
    }
}

/// The `set_subagent_subscription` line for `level`.
pub fn command(level: &str) -> String {
    format!(r#"{{"type":"set_subagent_subscription","level":"{level}"}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omp_runs_at_the_highest_level_any_window_needs() {
        let claims = SubscriptionClaims::default();
        assert_eq!(claims.claim("s1", "main", "progress"), Ok("progress"));
        assert_eq!(
            claims.claim("s1", "panel-office-01", "events"),
            Ok("events")
        );
        // The main window stops inspecting: the Office window still needs events.
        assert_eq!(claims.claim("s1", "main", "progress"), Ok("events"));
        assert_eq!(
            claims.claim("s1", "panel-office-01", "progress"),
            Ok("progress")
        );
    }

    #[test]
    fn a_closed_window_gives_its_claims_back() {
        let claims = SubscriptionClaims::default();
        claims.claim("s1", "main", "progress").unwrap();
        claims.claim("s1", "panel-office-01", "events").unwrap();
        claims.claim("s2", "panel-office-01", "events").unwrap();
        let mut changed = claims.release("panel-office-01");
        changed.sort();
        assert_eq!(
            changed,
            vec![("s1".to_string(), "progress"), ("s2".to_string(), "off")]
        );
        assert_eq!(claims.release("panel-office-01"), Vec::new());
    }

    #[test]
    fn an_unknown_level_is_refused() {
        assert!(SubscriptionClaims::default()
            .claim("s1", "main", "everything")
            .is_err());
    }
}
