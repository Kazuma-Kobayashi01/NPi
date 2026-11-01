use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

/// How long the takeover visuals stay up after the last observed operation.
/// A foreground takeover is a continuous share, not a per-click flash, so the
/// border, pointer and exit button must outlive the individual activity frame.
pub(super) const HOLD: Duration = Duration::from_secs(45);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Target {
    pub kind: String,
    pub id: String,
    pub app: Option<String>,
    pub title: Option<String>,
    pub bounds: Bounds,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Activity {
    pub id: String,
    pub phase: String,
    pub action: String,
    pub target: Option<Target>,
    pub cursor: Option<Point>,
    pub owner: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Display {
    pub session_id: String,
    pub activity: Activity,
    pub revision: u64,
    pub accent: String,
}

#[derive(Default)]
pub(super) struct Machine {
    /// Latest activity per (session, activity id), newest revision wins.
    active: HashMap<(String, String), (u64, Activity)>,
    /// Sticky geometry: the takeover keeps showing this until it expires.
    target: Option<Target>,
    cursor: Option<Point>,
    action: String,
    hold_until: Option<SystemTime>,
    session: Option<String>,
    pub revision: u64,
    pub accent: String,
}

impl Machine {
    /// Starts with the product accent until the active theme reports its own.
    pub fn with_accent(accent: &str) -> Self {
        Self { accent: accent.to_owned(), ..Self::default() }
    }

    pub fn observe(&mut self, session: &str, activity: Activity) {
        let finished = activity.phase == "finished";
        let key = (session.to_owned(), activity.id.clone());
        match activity.phase.as_str() {
            "finished" => {
                self.active.remove(&key);
            }
            "started" => {
                self.revision += 1;
                self.active.insert(key, (self.revision, activity));
            }
            "updated" => {
                // An update from an operation cleared by Stop must not resurrect it.
                if let Some((_, previous)) = self.active.get_mut(&key) {
                    *previous = activity;
                }
            }
            _ => unreachable!("activity phase is validated at the RPC boundary"),
        }
        // Sticky: whatever the operation last touched stays on screen.
        let latest = self.latest_id(session);
        if let Some((_, activity)) = self.active.get(&(session.to_owned(), latest)) {
            if let Some(target) = activity.target.clone() {
                self.target = Some(target);
            }
            if let Some(cursor) = activity.cursor {
                self.cursor = Some(cursor);
            }
            self.action = activity.action.clone();
        }
        self.hold_until = Some(SystemTime::now() + HOLD);
        if !finished {
            self.session = Some(session.to_owned());
        }
        self.revision += 1;
    }

    fn latest_id(&self, session: &str) -> String {
        self.active
            .iter()
            .filter(|((sid, _), _)| sid == session)
            .max_by_key(|(_, (order, _))| *order)
            .map(|((_, id), _)| id.clone())
            .unwrap_or_default()
    }

    pub fn clear(&mut self, session: &str) {
        self.active.retain(|(sid, _), _| sid != session);
        self.target = None;
        self.cursor = None;
        self.hold_until = None;
        self.revision += 1;
    }

    /// Whether the takeover visuals should still be on screen.
    pub fn holding(&self) -> bool {
        self.hold_until.is_some_and(|until| SystemTime::now() < until)
    }

    /// True once a share that was showing has lapsed; drives the retire tick.
    pub fn expired_without_activity(&self) -> bool {
        self.hold_until.is_some() && !self.holding()
    }

    pub fn display(&self) -> Option<Display> {
        if !self.holding() {
            return None;
        }
        let session = self.session.clone()?;
        let latest = self.active.iter().max_by_key(|(_, (order, _))| order);
        let activity = Activity {
            id: latest.map(|((_, id), _)| id.clone()).unwrap_or_else(|| "share".to_owned()),
            phase: "started".to_owned(),
            action: self.action.clone(),
            target: self.target.clone(),
            cursor: self.cursor,
            owner: latest.and_then(|(_, (_, activity))| activity.owner.clone()),
        };
        Some(Display {
            session_id: session,
            activity,
            revision: self.revision,
            accent: self.accent.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(id: &str, phase: &str) -> Activity {
        Activity {
            id: id.into(),
            phase: phase.into(),
            action: "click".into(),
            target: None,
            cursor: None,
            owner: None,
        }
    }

    #[test]
    fn an_older_finish_never_clears_a_newer_session() {
        let mut machine = Machine::default();
        machine.observe("one", activity("a", "started"));
        machine.observe("two", activity("b", "started"));
        machine.observe("one", activity("a", "finished"));
        assert_eq!(machine.display().unwrap().session_id, "two");
    }

    #[test]
    fn a_finished_operation_keeps_the_visuals_for_the_hold_window() {
        let mut machine = Machine::default();
        machine.observe("one", activity("a", "started"));
        machine.observe("one", activity("a", "finished"));
        assert!(machine.display().is_some(), "the takeover share is continuous");
        machine.clear("one");
        assert!(machine.display().is_none(), "stop clears it at once");
    }

    #[test]
    fn geometry_is_sticky_across_operations() {
        let mut machine = Machine::default();
        let mut first = activity("a", "started");
        first.target = Some(Target {
            kind: "window".into(),
            id: "7".into(),
            app: Some("Word".into()),
            title: None,
            bounds: Bounds { x: 1.0, y: 2.0, width: 30.0, height: 40.0 },
        });
        machine.observe("one", first);
        let mut second = activity("b", "started");
        second.action = "typeText".into();
        machine.observe("one", second);
        let display = machine.display().unwrap();
        assert_eq!(display.activity.target.unwrap().id, "7");
        assert_eq!(display.activity.action, "typeText");
    }
}
