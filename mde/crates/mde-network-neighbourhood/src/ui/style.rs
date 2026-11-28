use gtk4::gdk::Display;
use gtk4::CssProvider;
use mde_theme::{Palette, ACCENT_DEFAULT, FONT, FONT_MONO};

pub fn load(density: &str) {
    let theme = Palette::load();
    let accent = if theme.accent.is_empty() {
        ACCENT_DEFAULT
    } else {
        theme.accent.as_str()
    };
    let dark = theme.dark;
    // Surfaces come from the active MDE theme so this app matches Music, Files and Settings.
    let (bg, surface, surface2, text, muted, border, card) = (
        theme.bg.as_str(),
        theme.bg.as_str(),
        theme.bg_elevated.as_str(),
        theme.fg.as_str(),
        theme.fg_muted.as_str(),
        theme.border.as_str(),
        theme.bg_elevated.as_str(),
    );
    let danger = if theme.danger.is_empty() { "#d13438" } else { theme.danger.as_str() };
    let ok = if dark { "#6ccb5f" } else { "#107c10" };
    let radius = mde_theme::FLYOUT_RADIUS as i32;
    let (pad, row_min) = match density {
        "compact" => (6, 40),
        "dense" => (4, 36),
        _ => (10, 56),
    };

    let css = format!(
        r#"
window.nn-window {{
  background-color: {bg};
  color: {text};
  font-family: "{font}", "Segoe UI", sans-serif;
  font-size: 14px;
}}
.nn-sidebar {{
  background-color: {surface};
  border-right: 1px solid {border};
  min-width: 220px;
}}
.nn-nav-title {{
  font-size: 12px;
  font-weight: 600;
  color: {muted};
  letter-spacing: 0.04em;
  margin: 12px 16px 6px 16px;
}}
.nn-nav-btn {{
  border-radius: 4px;
  margin: 2px 8px;
  padding: 8px 12px;
  background: transparent;
  border: none;
  box-shadow: none;
}}
.nn-nav-btn:hover {{
  background-color: alpha({accent}, 0.08);
}}
.nn-nav-btn.nn-active {{
  background-color: alpha({accent}, 0.14);
  color: {accent};
  font-weight: 600;
}}
.nn-commandbar {{
  background-color: {surface2};
  border-bottom: 1px solid {border};
  padding: 6px 10px;
}}
.nn-commandbar button {{
  border-radius: 4px;
  padding: 6px 12px;
  margin: 0 2px;
}}
.nn-accent, button.nn-accent {{
  background-color: {accent};
  background-image: none;
  color: #ffffff;
  text-shadow: none;
  border: none;
  border-radius: 4px;
  padding: 8px 16px;
  font-weight: 600;
}}
.nn-accent:hover, button.nn-accent:hover {{
  background-color: {accent};
  background-image: none;
  opacity: 0.9;
}}
button.nn-accent:disabled {{
  opacity: 0.5;
}}
.nn-card {{
  background-color: {card};
  border: 1px solid {border};
  border-radius: {radius}px;
  border-left: 4px solid {border};
  padding: {pad}px;
  margin: 6px 12px;
  min-height: {row_min}px;
}}
.nn-card.nn-status-ok {{
  border-left-color: {ok};
}}
.nn-card.nn-status-err {{
  border-left-color: {danger};
}}
.nn-card.nn-status-off {{
  border-left-color: {muted};
}}
.nn-chip {{
  border-radius: 999px;
  padding: 1px 10px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  background-color: alpha({accent}, 0.12);
  color: {accent};
}}
.nn-connect-dialog {{
  background-color: {card};
  color: {text};
}}
.nn-card.nn-offline {{
  opacity: 0.6;
}}
.nn-card:hover {{
  border-color: {accent};
}}
.nn-hero-title {{
  font-size: 28px;
  font-weight: 600;
  margin-bottom: 8px;
}}
.nn-hero-sub {{
  color: {muted};
  font-size: 14px;
  margin-bottom: 20px;
}}
.nn-path {{
  font-family: "{mono}", monospace;
  font-size: 13px;
  color: {accent};
}}
.nn-badge {{
  border-radius: 4px;
  padding: 2px 8px;
  font-size: 12px;
  font-weight: 600;
  background-color: alpha({accent}, 0.12);
  color: {accent};
}}
.nn-badge-off {{
  background-color: alpha({danger}, 0.12);
  color: {danger};
}}
.nn-badge-ok {{
  background-color: alpha({ok}, 0.14);
  color: {ok};
}}
.nn-overlay {{
  background-color: alpha(#000000, 0.45);
}}
.nn-overlay-panel {{
  background-color: {card};
  border-radius: 8px;
  padding: 24px;
  min-width: 480px;
  max-width: 720px;
}}
.nn-log {{
  font-family: "{mono}", monospace;
  font-size: 12px;
  background-color: {surface};
  border: 1px solid {border};
  border-radius: 4px;
  padding: 8px;
}}
.nn-split-pane {{
  border: 1px solid {border};
  border-radius: 8px;
  background-color: {card};
  padding: 12px;
}}
.nn-conflict {{
  color: {danger};
  font-weight: 600;
}}
.nn-example {{
  color: {muted};
  font-family: "{mono}", monospace;
  font-size: 12px;
}}
.nn-connect-status {{
  margin-top: 4px;
  padding-top: 4px;
  border-top: 1px solid {border};
}}
.nn-connect-phrase {{
  font-size: 13px;
  font-weight: 600;
}}
.nn-connect-step {{
  font-size: 12px;
  color: {muted};
  font-family: "{mono}", monospace;
}}
.nn-connect-progress {{
  margin-top: 2px;
  margin-bottom: 2px;
}}
"#,
        bg = bg,
        surface = surface,
        surface2 = surface2,
        text = text,
        muted = muted,
        border = border,
        card = card,
        accent = accent,
        font = FONT,
        mono = FONT_MONO,
        pad = pad,
        row_min = row_min,
        danger = danger,
        ok = ok,
        radius = radius,
    );

    let provider = CssProvider::new();
    provider.load_from_string(&css);
    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
