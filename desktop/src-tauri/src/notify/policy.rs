//! When a notification is worth sending, and what it says. Pure: the
//! state that feeds it (window focus, the active tab, run times) lives in
//! `mod.rs`.

use std::time::Duration;

/// A run shorter than this finishes before the user has looked away.
pub const MIN_RUN: Duration = Duration::from_secs(8);
/// One notification per tab and kind in this window: a burst is one.
pub const DEDUPE: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A turn finished after a long run.
    Done,
    /// A question or approval waits for the user.
    NeedsInput,
    /// A request failed for good, or the agent process died.
    Failed,
    /// The Settings button: always shown, whatever the focus.
    Test,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    ZhCn,
    ZhTw,
    EnUs,
    JaJp,
}

impl Lang {
    /// The four languages the app speaks; anything else is `None`.
    pub fn parse(tag: &str) -> Option<Self> {
        match tag {
            "zh-CN" => Some(Self::ZhCn),
            "zh-TW" => Some(Self::ZhTw),
            "en-US" => Some(Self::EnUs),
            "ja-JP" => Some(Self::JaJp),
            _ => None,
        }
    }
}

/// What the user switched on in Settings: one switch per field.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub done: bool,
    pub ask: bool,
    pub failed: bool,
    pub lang: Lang,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            done: true,
            ask: true,
            failed: true,
            lang: Lang::EnUs,
        }
    }
}

/// What is known about the moment.
#[derive(Clone, Copy, Debug)]
pub struct Moment {
    /// The main window has the keyboard focus.
    pub window_focused: bool,
    /// The event's tab is the one on screen.
    pub tab_active: bool,
    /// How long the run took (`Done` only).
    pub run: Option<Duration>,
    /// Since this tab last sent this kind.
    pub since_last: Option<Duration>,
}

pub fn should_send(cfg: Config, kind: Kind, now: Moment) -> bool {
    if kind == Kind::Test {
        return true;
    }
    let category = match kind {
        Kind::Done => cfg.done,
        Kind::NeedsInput => cfg.ask,
        Kind::Failed => cfg.failed,
        Kind::Test => true,
    };
    if !cfg.enabled || !category {
        return false;
    }
    // Someone looking at that very tab needs no banner.
    if now.window_focused && now.tab_active {
        return false;
    }
    if kind == Kind::Done && now.run.is_none_or(|r| r < MIN_RUN) {
        return false;
    }
    now.since_last.is_none_or(|d| d >= DEDUPE)
}

/// The generic body of `kind`: never a prompt, reply or file name.
pub const fn body(kind: Kind, lang: Lang) -> &'static str {
    match (kind, lang) {
        (Kind::Done, Lang::ZhCn) => "任务已完成。",
        (Kind::Done, Lang::ZhTw) => "任務已完成。",
        (Kind::Done, Lang::EnUs) => "The task has finished.",
        (Kind::Done, Lang::JaJp) => "タスクが完了しました。",
        (Kind::NeedsInput, Lang::ZhCn) => "有一个问题或审批在等你处理。",
        (Kind::NeedsInput, Lang::ZhTw) => "有一個問題或審批在等你處理。",
        (Kind::NeedsInput, Lang::EnUs) => "A question or approval is waiting for you.",
        (Kind::NeedsInput, Lang::JaJp) => "確認または承認があなたを待っています。",
        (Kind::Failed, Lang::ZhCn) => "请求失败或会话已结束。",
        (Kind::Failed, Lang::ZhTw) => "請求失敗或工作階段已結束。",
        (Kind::Failed, Lang::EnUs) => "A request failed or the session ended.",
        (Kind::Failed, Lang::JaJp) => "リクエストが失敗したか、セッションが終了しました。",
        (Kind::Test, Lang::ZhCn) => "通知已就绪。",
        (Kind::Test, Lang::ZhTw) => "通知已就緒。",
        (Kind::Test, Lang::EnUs) => "Notifications are working.",
        (Kind::Test, Lang::JaJp) => "通知は正常に動作しています。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Config {
        Config {
            enabled: true,
            ..Config::default()
        }
    }

    fn away() -> Moment {
        Moment {
            window_focused: false,
            tab_active: true,
            run: Some(Duration::from_secs(30)),
            since_last: None,
        }
    }

    #[test]
    fn nothing_is_sent_while_the_master_switch_is_off() {
        assert!(!should_send(Config::default(), Kind::NeedsInput, away()));
    }

    #[test]
    fn each_category_has_its_own_switch() {
        let cfg = Config {
            done: false,
            ..on()
        };
        assert!(!should_send(cfg, Kind::Done, away()));
        assert!(should_send(cfg, Kind::NeedsInput, away()));
        let cfg = Config {
            ask: false,
            failed: false,
            ..on()
        };
        assert!(!should_send(cfg, Kind::NeedsInput, away()));
        assert!(!should_send(cfg, Kind::Failed, away()));
    }

    #[test]
    fn a_focused_window_on_that_tab_needs_no_banner_but_another_tab_does() {
        let here = Moment {
            window_focused: true,
            tab_active: true,
            ..away()
        };
        assert!(!should_send(on(), Kind::NeedsInput, here));
        let other_tab = Moment {
            window_focused: true,
            tab_active: false,
            ..away()
        };
        assert!(should_send(on(), Kind::NeedsInput, other_tab));
    }

    #[test]
    fn a_short_run_is_not_worth_a_done_banner() {
        let quick = Moment {
            run: Some(Duration::from_secs(3)),
            ..away()
        };
        assert!(!should_send(on(), Kind::Done, quick));
        assert!(!should_send(
            on(),
            Kind::Done,
            Moment {
                run: None,
                ..away()
            }
        ));
        assert!(should_send(on(), Kind::Failed, quick));
    }

    #[test]
    fn a_burst_is_one_notification_per_tab_and_kind() {
        let soon = Moment {
            since_last: Some(Duration::from_secs(5)),
            ..away()
        };
        assert!(!should_send(on(), Kind::NeedsInput, soon));
        let later = Moment {
            since_last: Some(DEDUPE),
            ..away()
        };
        assert!(should_send(on(), Kind::NeedsInput, later));
    }

    #[test]
    fn the_test_button_ignores_every_rule() {
        let focused = Moment {
            window_focused: true,
            tab_active: true,
            run: None,
            since_last: Some(Duration::ZERO),
        };
        assert!(should_send(Config::default(), Kind::Test, focused));
    }

    #[test]
    fn languages_parse_exactly_and_every_body_is_generic_text() {
        assert_eq!(Lang::parse("ja-JP"), Some(Lang::JaJp));
        assert_eq!(Lang::parse("fr-FR"), None);
        for lang in [Lang::ZhCn, Lang::ZhTw, Lang::EnUs, Lang::JaJp] {
            for kind in [Kind::Done, Kind::NeedsInput, Kind::Failed, Kind::Test] {
                assert!(!body(kind, lang).is_empty());
            }
        }
    }
}
