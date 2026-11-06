use hashbrown::HashMap;
use serde::Deserialize;

use crate::audio::Sound;
use crate::config::inclusivities::{
    Inclusivities, is_user_channel_server_included,
};
use crate::isupport;
use crate::serde::deserialize_string_or_vec;
use crate::server::Server;
use crate::target::Channel;
use crate::user::User;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Notification {
    pub show_toast: bool,
    pub request_attention: bool,
    pub show_content: bool,
    #[serde(deserialize_with = "deserialize_string_or_vec")]
    pub sound: Vec<String>,
    pub delay: Option<u32>,
    pub exclude: Option<Inclusivities>,
    pub include: Option<Inclusivities>,
}

impl Default for Notification {
    fn default() -> Self {
        Self {
            show_toast: false,
            request_attention: false,
            show_content: false,
            sound: Vec::new(),
            delay: Some(500),
            exclude: None,
            include: None,
        }
    }
}

impl Notification {
    pub fn should_notify(
        &self,
        user: &User,
        channel: Option<&Channel>,
        server: &Server,
        casemapping: isupport::CaseMap,
    ) -> bool {
        is_user_channel_server_included(
            self.include.as_ref(),
            self.exclude.as_ref(),
            user.nickname(),
            channel,
            server,
            casemapping,
        )
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Notifications {
    pub connected: Notification,
    pub disconnected: Notification,
    pub reconnected: Notification,
    pub direct_message: Notification,
    pub highlight: Notification,
    pub file_transfer_request: Notification,
    pub monitored_online: Notification,
    pub monitored_offline: Notification,
    #[serde(rename = "channel")]
    pub channels: HashMap<String, Notification>,
    pub reaction: Notification,
}

impl Notifications {
    /// Settings for messages in `channel`: an exact `[notifications.channel."<name>"]` entry,
    /// otherwise the most specific (longest) key with `*` / `?` wildcards that matches it,
    /// e.g. `"#chat/*"`.
    pub fn channel(&self, channel: &str) -> Option<&Notification> {
        self.channels.get(channel).or_else(|| {
            self.channels
                .iter()
                .filter(|(pattern, _)| {
                    pattern.contains(['*', '?']) && glob_match(pattern, channel)
                })
                .max_by_key(|(pattern, _)| pattern.len())
                .map(|(_, notification)| notification)
        })
    }

    pub fn load_sounds<'a>(
        &self,
        highlight_matches_sounds: impl Iterator<Item = &'a str>,
    ) -> HashMap<String, Sound> {
        let mut sounds = HashMap::new();

        // Helper function to load a sound and add it to the map
        let mut load_and_insert = |name: &str| {
            if !sounds.contains_key(name) {
                match Sound::load(name) {
                    Ok(sound) => {
                        sounds.insert(name.to_string(), sound);
                    }
                    Err(e) => {
                        log::warn!("Failed to load sound '{name}': {e}");
                    }
                }
            }
        };

        // Load sounds from each notification
        let notifications = [
            &self.connected,
            &self.disconnected,
            &self.reconnected,
            &self.direct_message,
            &self.highlight,
            &self.file_transfer_request,
            &self.monitored_online,
            &self.monitored_offline,
            &self.reaction,
        ];

        for sound_name in notifications
            .into_iter()
            .chain(self.channels.values())
            .flat_map(|notification| &notification.sound)
        {
            load_and_insert(sound_name);
        }

        for sound_name in highlight_matches_sounds {
            load_and_insert(sound_name);
        }

        sounds
    }
}

/// `*` matches any run of characters (including none), `?` exactly one; everything else literally.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // Position of the last `*` and the text position it was tried at, to backtrack to
    let mut star: Option<(usize, usize)> = None;

    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                // Let the last `*` swallow one more character and retry
                Some((star_p, star_t)) => {
                    p = star_p + 1;
                    t = star_t + 1;
                    star = Some((star_p, star_t + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn glob() {
        assert!(glob_match("#chat/*", "#chat/alice"));
        assert!(glob_match("#chat/*", "#chat/"));
        assert!(!glob_match("#chat/*", "#team/general"));
        assert!(glob_match("#*/general", "#team/general"));
        assert!(glob_match("#chat/?lice", "#chat/alice"));
        assert!(!glob_match("#chat/?", "#chat/alice"));
        assert!(glob_match("*", "#anything"));
        assert!(glob_match("#a*b*c", "#axxbyyc"));
        assert!(!glob_match("#a*b*c", "#axxbyy"));
        assert!(glob_match("#chat/alice", "#chat/alice"));
    }
}
