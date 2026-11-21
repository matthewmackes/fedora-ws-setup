//! Settings page bodies keyed by registry id.

mod accounts;
mod advanced;
mod apps_page;
mod devices;
mod gaming;
mod home;
mod network;
mod personal;
mod privacy;
pub mod system_pages;
mod time_lang;
mod update;
mod accessibility;
mod tools;

use iced::Element;

use crate::{Control, Message};

pub fn view(app: &Control) -> Element<'_, Message> {
    match app.page.as_str() {
        "home" => home::view(app),
        "system" => system_pages::hub(app),
        "display" => system_pages::display(app),
        "nightlight" => system_pages::nightlight(app),
        "sound" => system_pages::sound(app),
        "mixer" => system_pages::mixer(app),
        "notifications" => system_pages::notifications(app),
        "notifapps" => tools::notif_apps(app),
        "focus" => system_pages::focus(app),
        "power" => system_pages::power(app),
        "storage" => system_pages::storage(app),
        "clipboard" => system_pages::clipboard(app),
        "tools" => tools::hub(app),
        "trayicons" => tools::tray_icons(app),
        "about" => system_pages::about(app),
        "devices" => devices::hub(app),
        "bluetooth" => devices::bluetooth(app),
        "printers" => devices::printers(app),
        "mouse" => devices::mouse(app),
        "touchpad" => devices::touchpad(app),
        "network" => network::hub(app),
        "wifi" => network::wifi(app),
        "ethernet" => network::ethernet(app),
        "vpn" => network::vpn(app),
        "proxy" => network::proxy(app),
        "nearby" => network::nearby(app),
        "personalization" => personal::hub(app),
        "background" => personal::background(app),
        "colors" => personal::colors(app),
        "themes" => personal::themes(app),
        "lockscreen" => personal::lockscreen(app),
        "fonts" => personal::fonts(app),
        "windowlayout" => personal::window_layout(app),
        "start" => personal::start(app),
        "taskbar" => personal::taskbar(app),
        "widgets" => personal::widgets(app),
        "apps" => apps_page::hub(app),
        "installed" => apps_page::installed(app),
        "defaultapps" => apps_page::defaults(app),
        "startup" => apps_page::startup(app),
        "accounts" => accounts::hub(app),
        "yourinfo" => accounts::yourinfo(app),
        "signin" => accounts::signin(app),
        "otherusers" => accounts::other_users(app),
        "time" => time_lang::hub(app),
        "datetime" => time_lang::datetime(app),
        "language" => time_lang::language(app),
        "typing" => time_lang::typing(app),
        "gaming" => gaming::hub(app),
        "gamemode" => gaming::gamemode(app),
        "captures" => gaming::captures(app),
        "accessibility" => accessibility::hub(app),
        "textsize" => accessibility::textsize(app),
        "visual" => accessibility::visual(app),
        "pointer" => accessibility::pointer(app),
        "contrast" => accessibility::contrast(app),
        "shortcuts" => accessibility::shortcuts(app),
        "privacy" => privacy::hub(app),
        "security" | "firewall" => privacy::security(app),
        "camera" => privacy::camera(app),
        "microphone" => privacy::microphone(app),
        "location" => privacy::location(app),
        "activity" => privacy::activity(app),
        "update" => update::hub(app),
        "updatehistory" => update::history(app),
        "advanced" => advanced::hub(app),
        "mmc" => advanced::mmc(app),
        "config" => advanced::editor(app),
        _ => home::view(app),
    }
}
