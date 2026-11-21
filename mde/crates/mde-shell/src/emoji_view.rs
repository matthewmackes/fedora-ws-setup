//! W426 / A068: emoji picker — grid + recents + search (wtype / wl-copy; gnome-characters overflow).

use super::*;
use iced::widget::column;
use std::path::PathBuf;

/// Common emoji for the grid (kept ASCII-comment free for source encoding).
pub(crate) const COMMON_EMOJI: &[&str] = &[
    "😀", "😃", "😄", "😁", "😆", "😅", "🤣", "😂", "🙂", "😉", "😊", "😇", "🥰", "😍",
    "🤩", "😘", "😗", "😚", "😋", "😜", "🤪", "😝", "🤑", "🤗", "🤭", "🤫", "🤔", "🤐",
    "🤨", "😐", "😑", "😶", "🙄", "😏", "😣", "😥", "😮", "😯", "😪", "😫", "🥱", "😴",
    "😌", "😛", "😓", "😕", "🙃", "🫠", "😲", "☹️", "🙁", "😖", "😞", "😟", "😤", "😢",
    "😭", "😦", "😧", "😨", "😩", "🤯", "😬", "😰", "😱", "🥵", "🥶", "😳", "🤪", "😵",
    "😡", "😠", "🤬", "😷", "🤒", "🤕", "🤢", "🤮", "🥴", "👋", "🤚", "🖐️", "✋", "🖖",
    "👌", "🤌", "🤏", "✌️", "🤞", "🫰", "🤟", "🤘", "🤙", "👈", "👉", "👆", "👇", "☝️",
    "👍", "👎", "✊", "👊", "🤛", "🤜", "👏", "🙌", "🫶", "👐", "🤲", "🤝", "🙏", "💪",
    "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔", "❣️", "💕", "💞", "💓",
    "💗", "💖", "💘", "💝", "⭐", "🌟", "✨", "⚡", "🔥", "💥", "🎉", "🎊", "🎈", "🎁",
    "🏆", "🥇", "🎯", "🎮", "🎵", "🎶", "💡", "📌", "📎", "🔗", "✅", "❌", "❓", "❗",
    "💯", "🚀", "✈️", "🏠", "💻", "📱", "⌨️", "🖱️", "📷", "🕐", "📅", "📁", "📂", "📝",
];

/// Simple keyword → emoji hints for search (A068).
const SEARCH_HINTS: &[(&str, &[&str])] = &[
    ("smile", &["😀", "😃", "😄", "😁", "😆", "😊"]),
    ("happy", &["😀", "😄", "😁", "🎉"]),
    ("sad", &["😢", "😭", "😞", "☹️"]),
    ("heart", &["❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "💕"]),
    ("love", &["❤️", "😍", "🥰", "💕", "💘"]),
    ("fire", &["🔥", "💥"]),
    ("thumb", &["👍", "👎"]),
    ("ok", &["👌", "✅"]),
    ("clap", &["👏", "🙌"]),
    ("star", &["⭐", "🌟", "✨"]),
    ("party", &["🎉", "🎊", "🎈"]),
    ("check", &["✅"]),
    ("cross", &["❌"]),
    ("think", &["🤔", "🤨"]),
    ("wave", &["👋"]),
    ("pc", &["💻", "⌨️", "🖱️"]),
    ("phone", &["📱"]),
];

const COLS: usize = 10;
const RECENTS_MAX: usize = 24;

fn recents_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home)
        .join(".local/state/mde")
        .join("emoji-recents.txt")
}

pub(crate) fn load_recents() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(recents_path()) else {
        return Vec::new();
    };
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(RECENTS_MAX)
        .collect()
}

pub(crate) fn push_recent(emoji: &str) {
    let mut list = load_recents();
    list.retain(|e| e != emoji);
    list.insert(0, emoji.to_string());
    list.truncate(RECENTS_MAX);
    if let Some(parent) = recents_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = list.join("\n");
    let _ = std::fs::write(recents_path(), body);
}

fn search_matches(query: &str) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return COMMON_EMOJI.iter().map(|s| (*s).to_string()).collect();
    }
    if COMMON_EMOJI.iter().any(|e| *e == q.as_str()) {
        return vec![q];
    }
    let mut hits: Vec<String> = Vec::new();
    for (key, emojis) in SEARCH_HINTS {
        if key.contains(&q) || q.contains(key) {
            for e in *emojis {
                if !hits.iter().any(|h| h == e) {
                    hits.push((*e).to_string());
                }
            }
        }
    }
    if hits.is_empty() {
        COMMON_EMOJI
            .iter()
            .take(40)
            .map(|s| (*s).to_string())
            .collect()
    } else {
        hits
    }
}

/// Type emoji into the focused client, else copy for paste.
pub(crate) fn insert_emoji(emoji: &str) -> String {
    push_recent(emoji);
    if keyboard::type_token(emoji).is_ok() {
        return format!("Emoji · {emoji}");
    }
    let copied = std::process::Command::new("wl-copy")
        .arg(emoji)
        .spawn()
        .is_ok();
    if copied {
        format!("Emoji · copied {emoji}")
    } else {
        "Emoji · install wtype or wl-copy".into()
    }
}

/// Overflow to the system characters app when the grid is not enough.
pub(crate) fn spawn_emoji_overflow() -> String {
    for bin in ["gnome-characters", "smile"] {
        if std::process::Command::new(bin).spawn().is_ok() {
            return format!("Emoji · more · {bin}");
        }
    }
    "Emoji · install gnome-characters for more".into()
}

fn emoji_grid(emojis: &[String]) -> Element<'static, Message> {
    let mut grid = column![].spacing(4);
    let mut row_el = row![].spacing(4);
    for (i, emoji) in emojis.iter().enumerate() {
        let e = emoji.clone();
        let label = emoji.clone();
        row_el = row_el.push(
            quiet(text(label).size(18))
                .on_press(Message::InsertEmoji(e))
                .padding([6, 8]),
        );
        if (i + 1) % COLS == 0 {
            grid = grid.push(row_el);
            row_el = row![].spacing(4);
        }
    }
    if !emojis.is_empty() && emojis.len() % COLS != 0 {
        grid = grid.push(row_el);
    }
    grid.into()
}

impl Shell {
    pub(super) fn view_emoji(&self) -> Element<'_, Message> {
        let query = &self.emoji_query;
        let matches = search_matches(query);
        let recents = load_recents();
        let mut body = column![].spacing(8);
        if !recents.is_empty() && query.trim().is_empty() {
            body = body
                .push(text("Recents").size(12))
                .push(emoji_grid(&recents));
        }
        body = body
            .push(
                text(if query.trim().is_empty() {
                    "All"
                } else {
                    "Results"
                })
                .size(12),
            )
            .push(emoji_grid(&matches));

        let head = row![
            text("Emoji").size(16),
            Space::with_width(Fill),
            quiet(text("More…").size(12)).on_press(Message::EmojiOverflow),
            quiet(text("Close").size(12)).on_press(Message::Dismiss),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        let search = text_input("Search emoji…", query)
            .on_input(Message::EmojiQuery)
            .padding(8)
            .size(13);

        container(
            column![head, search, hairline(&self.palette), scrollable(body).height(Fill)]
                .spacing(8),
        )
        .padding(14)
        .width(Fill)
        .height(Fill)
        .into()
    }
}
