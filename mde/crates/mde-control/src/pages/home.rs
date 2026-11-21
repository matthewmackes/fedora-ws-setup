use iced::widget::{column, row, Space};
use iced::Element;

use crate::nav;
use crate::widgets::{choice, nav_card, page_frame, section_label, settings_row, status_row};
use crate::{system, Control, Message};

fn classic_left_prompt_dismissed(cfg: &mde_config::MdeConfig) -> bool {
    cfg.extras
        .get("shell.classic_left_prompt_dismissed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn view(app: &Control) -> Element<'_, Message> {
    let p = &app.palette;
    let shell = &app.cfg.active().shell;
    let left_active = shell.taskbar_left || shell.taskbar_align.eq_ignore_ascii_case("left");
    let mut body = column![
        status_row(p, "avatar-default", "Signed in as", &system::user_display_name()),
        status_row(
            p,
            "help-about",
            "Profile",
            &format!("{} · {}", app.cfg.active_profile, mde_theme::PRODUCT),
        ),
        Space::with_height(4),
        section_label(p, "Community presets"),
        settings_row(
            p,
            "object-align-left",
            "Classic left taskbar",
            Some("Community favourite · Start and pins on the left (default stays Center)"),
            if left_active {
                choice(p, "Applied", true, Message::Navigate("taskbar".into()))
            } else {
                choice(
                    p,
                    "Apply",
                    false,
                    Message::ApplyClassicLeftPreset,
                )
            },
        ),
        settings_row(
            p,
            "preferences-desktop",
            "Community Classic profile",
            Some("Left taskbar · no Widgets · search hidden · More pins"),
            choice(
                p,
                if app.cfg.active_profile == "community-classic" {
                    "Active"
                } else {
                    "Use"
                },
                app.cfg.active_profile == "community-classic",
                Message::SelectProfile("community-classic".into()),
            ),
        ),
        settings_row(
            p,
            "distributor-logo",
            "Stock Win11 profile",
            Some("Center taskbar · Widgets on · search icon · More pins"),
            choice(
                p,
                if app.cfg.active_profile == "stock-win11" {
                    "Active"
                } else {
                    "Use"
                },
                app.cfg.active_profile == "stock-win11",
                Message::SelectProfile("stock-win11".into()),
            ),
        ),
    ]
    .spacing(4);

    // A001 first-run prompt until applied or dismissed.
    if !classic_left_prompt_dismissed(&app.cfg) && !left_active {
        body = body.push(settings_row(
            p,
            "preferences-desktop",
            "Try Classic left?",
            Some("One-click left alignment · you can switch back anytime in Taskbar settings"),
            row![
                choice(p, "Apply", false, Message::ApplyClassicLeftPreset),
                choice(p, "Not now", false, Message::DismissClassicLeftPrompt),
            ]
            .spacing(8)
            .into(),
        ));
    }

    body = body.push(Space::with_height(4));
    body = body.push(section_label(p, "Security health"));
    for (title, detail, ok) in system::security_health_cards() {
        let nav = match title.as_str() {
            "Firewall" => "firewall",
            "SELinux" => "security",
            "Updates" => "update",
            "Storage" => "storage",
            _ => "privacy",
        };
        body = body.push(settings_row(
            p,
            if ok {
                "security-high"
            } else {
                "security-medium"
            },
            title,
            Some(if ok {
                format!("OK · {detail}")
            } else {
                format!("Attention · {detail}")
            }),
            choice(p, "Open", false, Message::Navigate(nav.into())),
        ));
    }

    body = body.push(Space::with_height(4));
    body = body.push(section_label(p, "Recommended"));

    for key in [
        "system",
        "bluetooth",
        "network",
        "personalization",
        "apps",
        "accounts",
        "update",
    ] {
        if let Some(page) = nav::get(key) {
            // Prefer category titles for recommended chips.
            let page = nav::get(match key {
                "bluetooth" => "devices",
                other => other,
            })
            .unwrap_or(page);
            body = body.push(nav_card(
                p,
                page.icon,
                page.title,
                page.caption,
                Message::Navigate(page.key.into()),
            ));
        }
    }

    if !app.search.trim().is_empty() {
        body = body.push(section_label(p, "Search results"));
        for hit in nav::search_hits(&app.search) {
            let parent = hit
                .parent
                .and_then(nav::get)
                .map(|x| x.title)
                .unwrap_or("");
            let caption = if parent.is_empty() {
                hit.caption.to_string()
            } else {
                format!("{parent} > {}", hit.caption)
            };
            body = body.push(nav_card(
                p,
                hit.icon,
                hit.title,
                caption,
                Message::Navigate(hit.key.into()),
            ));
        }
    }

    page_frame(p, "Home", Some("Your most used settings"), body.into())
}
