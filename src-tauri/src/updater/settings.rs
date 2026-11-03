//! Update preferences, stored in Nexdir's configuration folder (`update-settings.json`) so the
//! backend — not the WebView — decides whether an automatic check may contact the network.

use super::feed::Channel;
use serde::{Deserialize, Serialize};

pub const DEFAULT_INTERVAL_HOURS: u32 = 24;
pub const MIN_INTERVAL_HOURS: u32 = 1;
pub const MAX_INTERVAL_HOURS: u32 = 24 * 30;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateSettings {
    /// Off by default: Nexdir makes no update requests unless the user turns this on or
    /// clicks "Check for updates".
    pub auto_check: bool,
    /// `None` until the user picks one; then the running version decides (see `Channel::default_for`).
    pub channel: Option<Channel>,
    /// Show the in-app notice when an automatic check finds an update.
    pub notify: bool,
    /// Minimum time between automatic checks.
    pub interval_hours: u32,
    /// Unix milliseconds of the last check that reached the feed (successful or not).
    pub last_attempt: Option<i64>,
    /// Unix milliseconds of the last check that got a valid answer.
    pub last_success: Option<i64>,
    /// "Remind me later": no notice for this version before the given time.
    pub snoozed: Option<Snooze>,
    /// Version an installer was started for; read once at the next start to tell whether the
    /// update was installed.
    pub pending_install: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snooze {
    pub version: String,
    pub until: i64,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: false,
            channel: None,
            notify: true,
            interval_hours: DEFAULT_INTERVAL_HOURS,
            last_attempt: None,
            last_success: None,
            snoozed: None,
            pending_install: None,
        }
    }
}

/// The part of the settings the user edits; the bookkeeping fields are kept by the backend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdateSettings {
    pub auto_check: bool,
    pub channel: Channel,
    pub notify: bool,
    pub interval_hours: u32,
}

impl UpdateSettings {
    pub fn apply(&mut self, user: &UserUpdateSettings) {
        if self.channel != Some(user.channel) {
            // A snooze is about one announced version; a new channel starts fresh.
            self.snoozed = None;
        }
        self.auto_check = user.auto_check;
        self.channel = Some(user.channel);
        self.notify = user.notify;
        self.interval_hours = user.interval_hours.clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS);
    }

    /// Whether an automatic check is due at `now` (Unix ms). The interval counts from the last
    /// attempt, so an offline PC does not retry on every start.
    pub fn auto_check_due(&self, now: i64) -> bool {
        if !self.auto_check {
            return false;
        }
        let interval = i64::from(self.interval_hours.clamp(MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS)) * 3_600_000;
        match self.last_attempt {
            // A clock that went backwards must not block checks forever.
            Some(last) if last <= now => now - last >= interval,
            _ => true,
        }
    }

    pub fn is_snoozed(&self, version: &str, now: i64) -> bool {
        self.snoozed.as_ref().is_some_and(|s| s.version == version && now < s.until)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;

    #[test]
    fn defaults_are_private() {
        let s = UpdateSettings::default();
        assert!(!s.auto_check, "automatic checks are off by default");
        assert!(!s.auto_check_due(0));
        assert_eq!(s.interval_hours, 24);
    }

    #[test]
    fn automatic_check_runs_at_most_once_per_interval() {
        let mut s = UpdateSettings {
            auto_check: true,
            ..Default::default()
        };
        assert!(s.auto_check_due(100 * HOUR), "never checked");
        s.last_attempt = Some(100 * HOUR);
        assert!(!s.auto_check_due(100 * HOUR + 1));
        assert!(!s.auto_check_due(123 * HOUR));
        assert!(s.auto_check_due(124 * HOUR));
        // Clock moved back: check again instead of waiting until the old time comes round.
        assert!(s.auto_check_due(50 * HOUR));
        s.auto_check = false;
        assert!(!s.auto_check_due(1000 * HOUR));
    }

    #[test]
    fn interval_is_clamped() {
        let mut s = UpdateSettings::default();
        s.apply(&UserUpdateSettings {
            auto_check: true,
            channel: Channel::Stable,
            notify: true,
            interval_hours: 0,
        });
        assert_eq!(s.interval_hours, MIN_INTERVAL_HOURS);
        s.apply(&UserUpdateSettings {
            auto_check: true,
            channel: Channel::Stable,
            notify: true,
            interval_hours: 1_000_000,
        });
        assert_eq!(s.interval_hours, MAX_INTERVAL_HOURS);
    }

    #[test]
    fn switching_channel_clears_the_snooze() {
        let mut s = UpdateSettings {
            channel: Some(Channel::Preview),
            snoozed: Some(Snooze {
                version: "0.6.0".into(),
                until: 10,
            }),
            ..Default::default()
        };
        assert!(s.is_snoozed("0.6.0", 5));
        assert!(!s.is_snoozed("0.6.0", 10));
        assert!(!s.is_snoozed("0.6.1", 5));
        s.apply(&UserUpdateSettings {
            auto_check: false,
            channel: Channel::Preview,
            notify: true,
            interval_hours: 24,
        });
        assert!(s.snoozed.is_some());
        s.apply(&UserUpdateSettings {
            auto_check: false,
            channel: Channel::Stable,
            notify: true,
            interval_hours: 24,
        });
        assert!(s.snoozed.is_none());
    }

    #[test]
    fn old_or_partial_files_load_with_defaults() {
        let s: UpdateSettings = serde_json::from_str(r#"{"autoCheck":true}"#).unwrap();
        assert!(s.auto_check);
        assert_eq!(s.interval_hours, DEFAULT_INTERVAL_HOURS);
        assert!(s.notify);
    }
}
