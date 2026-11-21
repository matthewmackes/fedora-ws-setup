use iced::widget::{column, image, text};
use iced::Element;

use crate::widgets::{
    accent_btn, child_nav_rows, empty_state, hero, page_frame, section_label, status_row,
};
use crate::{system, Control, Message};

pub fn hub(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let name = system::user_display_name();
    let mut hero_col = column![].spacing(8);
    if let Some(path) = system::face_path() {
        hero_col = hero_col.push(image(path).width(96).height(96));
    } else {
        hero_col = hero_col.push(text(name.chars().next().unwrap_or('U').to_string()).size(48));
    }
    hero_col = hero_col.push(text(name).size(20));
    hero_col = hero_col.push(text(format!("Local account · {}", system::username())).size(13));
    page_frame(
        p,
        "Accounts",
        Some("Your info, sign-in options, other users"),
        column![hero(p, hero_col.into()), child_nav_rows(p, "accounts")]
            .spacing(4)
            .into(),
    )
}

pub fn yourinfo(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let name = system::user_display_name();
    let user = system::username();
    let mut hero_col = column![].spacing(8);
    if let Some(path) = system::face_path() {
        hero_col = hero_col.push(image(path).width(160).height(160));
    } else {
        hero_col = hero_col.push(text(name.chars().next().unwrap_or('U').to_string()).size(64));
    }
    hero_col = hero_col.push(text(name.clone()).size(22));
    hero_col = hero_col.push(text(format!("Local account · {user}")).size(13));
    page_frame(
        p,
        "Your info",
        Some("Profile photo, name"),
        column![
            hero(p, hero_col.into()),
            section_label(p, "Account settings"),
            status_row(p, "avatar-default", "Your name", &name),
            status_row(p, "system-users", "Username", &user),
            accent_btn(p, "Choose a new picture", Message::PickAccountPicture),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn signin(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    page_frame(
        p,
        "Sign-in options",
        Some("Password, fingerprint, automatic sign-in"),
        column![
            status_row(p, "dialog-password", "Account", &system::username()),
            empty_state(
                p,
                "Password",
                "Change your password with passwd in a terminal. Polkit prompts as needed.",
                None,
            ),
            empty_state(
                p,
                "Fingerprint",
                "Use fprintd when a reader is available.",
                None,
            ),
            empty_state(
                p,
                "Automatic sign-in",
                "Configured in the display manager (GDM/SDDM/mde-greeter).",
                None,
            ),
        ]
        .spacing(4)
        .into(),
    )
}

pub fn other_users(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let users = system::list_users();
    let mut col = column![
        section_label(p, "Users on this PC"),
        status_row(
            p,
            "system-lock-screen",
            "Fast user switching",
            "Single-user lock only · lock then sign in as another account at the greeter",
        ),
    ]
    .spacing(4);
    if users.trim().is_empty() {
        col = col.push(empty_state(
            p,
            "No local users listed",
            "Add accounts with useradd or your distribution’s Users tool.",
            None,
        ));
    } else {
        for line in users.lines() {
            col = col.push(status_row(p, "system-users", line, "Local"));
        }
    }
    page_frame(
        p,
        "Other users",
        Some("Add, remove and manage other user accounts"),
        col.into(),
    )
}
