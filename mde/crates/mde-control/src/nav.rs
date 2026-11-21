//! Hierarchical Settings page registry (Win11 IA + Advanced).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageDef {
    pub key: &'static str,
    pub title: &'static str,
    pub parent: Option<&'static str>,
    pub icon: &'static str,
    pub caption: &'static str,
    pub keywords: &'static str,
    pub aliases: &'static [&'static str],
}

/// Top-level sidebar categories in Win11 order, plus Advanced.
pub const CATEGORIES: &[PageDef] = &[
    PageDef {
        key: "home",
        title: "Home",
        parent: None,
        icon: "go-home",
        caption: "Your most used settings",
        keywords: "home start",
        aliases: &[],
    },
    PageDef {
        key: "system",
        title: "System",
        parent: None,
        icon: "computer",
        caption: "Display, sound, notifications, power",
        keywords: "system display sound power",
        aliases: &["system"],
    },
    PageDef {
        key: "devices",
        title: "Bluetooth & devices",
        parent: None,
        icon: "bluetooth",
        caption: "Bluetooth, printers, mouse, keyboard",
        keywords: "bluetooth devices printers mouse",
        aliases: &["bluetooth-devices"],
    },
    PageDef {
        key: "network",
        title: "Network & internet",
        parent: None,
        icon: "network-wireless",
        caption: "Wi-Fi, airplane mode, ethernet, VPN",
        keywords: "wifi network internet ethernet vpn",
        aliases: &["network-internet"],
    },
    PageDef {
        key: "personalization",
        title: "Personalization",
        parent: None,
        icon: "preferences-desktop-theme",
        caption: "Background, colours, themes, Start, taskbar",
        keywords: "personalization wallpaper theme",
        aliases: &[],
    },
    PageDef {
        key: "apps",
        title: "Apps",
        parent: None,
        icon: "view-grid",
        caption: "Installed apps, default apps, startup",
        keywords: "apps applications programs",
        aliases: &["appsfeatures", "applications"],
    },
    PageDef {
        key: "accounts",
        title: "Accounts",
        parent: None,
        icon: "system-users",
        caption: "Your info, sign-in options, other users",
        keywords: "accounts user login",
        aliases: &[],
    },
    PageDef {
        key: "time",
        title: "Time & language",
        parent: None,
        icon: "preferences-system-time",
        caption: "Date and time, language and region, typing",
        keywords: "time language region keyboard",
        aliases: &["time-language"],
    },
    PageDef {
        key: "gaming",
        title: "Gaming",
        parent: None,
        icon: "applications-games",
        caption: "Game Mode, captures",
        keywords: "gaming game mode screenshot",
        aliases: &[],
    },
    PageDef {
        key: "accessibility",
        title: "Accessibility",
        parent: None,
        icon: "preferences-desktop-accessibility",
        caption: "Text size, visual effects, contrast, keyboard",
        keywords: "accessibility ease of access",
        aliases: &["ease", "easeofaccess"],
    },
    PageDef {
        key: "privacy",
        title: "Privacy & security",
        parent: None,
        icon: "security-high",
        caption: "Windows Security, camera, microphone, location",
        keywords: "privacy security firewall camera",
        aliases: &[],
    },
    PageDef {
        key: "update",
        title: "Windows Update",
        parent: None,
        icon: "system-software-update",
        caption: "Check for updates, update history",
        keywords: "update windows update dnf flatpak",
        aliases: &["windowsupdate"],
    },
    PageDef {
        key: "advanced",
        title: "Advanced",
        parent: None,
        icon: "applications-system",
        caption: "System tools, Start folders, and apps moved off Start",
        keywords: "advanced mmc regedit config start folders apps",
        aliases: &[],
    },
];

const PAGES: &[PageDef] = &[
    // System
    PageDef {
        key: "display",
        title: "Display",
        parent: Some("system"),
        icon: "video-display",
        caption: "Monitors, brightness, night light, display profile",
        keywords: "screen monitor resolution scale refresh",
        aliases: &["screen", "displays"],
    },
    PageDef {
        key: "nightlight",
        title: "Night light",
        parent: Some("display"),
        icon: "weather-clear-night",
        caption: "Warmer colours at night",
        keywords: "blue light warm night",
        aliases: &["night-light", "blue-light", "night"],
    },
    PageDef {
        key: "sound",
        title: "Sound",
        parent: Some("system"),
        icon: "audio-volume-high",
        caption: "Volume levels, output, input, sound devices",
        keywords: "audio speaker headphones microphone volume",
        aliases: &["audio"],
    },
    PageDef {
        key: "mixer",
        title: "Volume mixer",
        parent: Some("sound"),
        icon: "multimedia-volume-control",
        caption: "App volume mix",
        keywords: "app volume per-app mixer",
        aliases: &["apps-volume"],
    },
    PageDef {
        key: "notifications",
        title: "Notifications",
        parent: Some("system"),
        icon: "preferences-system-notifications",
        caption: "Alerts from apps and system, do not disturb",
        keywords: "alerts toast dnd banner",
        aliases: &[],
    },
    PageDef {
        key: "focus",
        title: "Focus",
        parent: Some("system"),
        icon: "alarm",
        caption: "Focus sessions, distraction settings",
        keywords: "timer concentrate quiet hours",
        aliases: &["focusassist", "quiethours"],
    },
    PageDef {
        key: "power",
        title: "Power & battery",
        parent: Some("system"),
        icon: "battery",
        caption: "Sleep, battery usage, power mode",
        keywords: "battery sleep idle lock suspend",
        aliases: &["powersleep", "batterysaver"],
    },
    PageDef {
        key: "storage",
        title: "Storage",
        parent: Some("system"),
        icon: "drive-harddisk",
        caption: "Storage space, drives, temporary files",
        keywords: "disk space free trash",
        aliases: &["storagesense"],
    },
    PageDef {
        key: "clipboard",
        title: "Clipboard",
        parent: Some("system"),
        icon: "edit-copy",
        caption: "Cut and copy history",
        keywords: "copy paste history",
        aliases: &[],
    },
    PageDef {
        key: "about",
        title: "About",
        parent: Some("system"),
        icon: "help-about",
        caption: "MDE branding, device specs, open source licenses",
        keywords: "pc name hostname specs processor ram mde license fedora wayland",
        aliases: &[],
    },
    // Devices
    PageDef {
        key: "bluetooth",
        title: "Bluetooth",
        parent: Some("devices"),
        icon: "bluetooth",
        caption: "Bluetooth devices and power",
        keywords: "bluetooth bluez pair",
        aliases: &[],
    },
    PageDef {
        key: "printers",
        title: "Printers & scanners",
        parent: Some("devices"),
        icon: "printer",
        caption: "Printers, scanners, and print queue",
        keywords: "printer cups scanner",
        aliases: &["printers"],
    },
    PageDef {
        key: "mouse",
        title: "Mouse",
        parent: Some("devices"),
        icon: "input-mouse",
        caption: "Buttons, pointer speed",
        keywords: "mouse pointer buttons",
        aliases: &[],
    },
    PageDef {
        key: "touchpad",
        title: "Touchpad",
        parent: Some("devices"),
        icon: "input-touchpad",
        caption: "Taps, scrolling, gestures",
        keywords: "touchpad trackpad gestures",
        aliases: &[],
    },
    // Network
    PageDef {
        key: "wifi",
        title: "Wi-Fi",
        parent: Some("network"),
        icon: "network-wireless",
        caption: "Connect, manage known networks",
        keywords: "wifi wireless ssid",
        aliases: &["wlan"],
    },
    PageDef {
        key: "ethernet",
        title: "Ethernet",
        parent: Some("network"),
        icon: "network-wired",
        caption: "Wired connections",
        keywords: "ethernet lan cable",
        aliases: &[],
    },
    PageDef {
        key: "vpn",
        title: "VPN",
        parent: Some("network"),
        icon: "network-vpn",
        caption: "VPN connections",
        keywords: "vpn wireguard openvpn",
        aliases: &[],
    },
    PageDef {
        key: "proxy",
        title: "Proxy",
        parent: Some("network"),
        icon: "preferences-system-network",
        caption: "Proxy server settings",
        keywords: "proxy http https",
        aliases: &[],
    },
    PageDef {
        key: "nearby",
        title: "Nearby Share",
        parent: Some("network"),
        icon: "network-wireless-connected-symbolic",
        caption: "KDE Connect pairing and devices",
        keywords: "nearby share kde connect pair phone",
        aliases: &["kdeconnect", "kde-connect"],
    },
    // Personalization
    PageDef {
        key: "background",
        title: "Background",
        parent: Some("personalization"),
        icon: "preferences-desktop-wallpaper",
        caption: "Background image, colour, Bing wallpaper",
        keywords: "wallpaper desktop picture bing spotlight",
        aliases: &["wallpaper"],
    },
    PageDef {
        key: "colors",
        title: "Colours",
        parent: Some("personalization"),
        icon: "preferences-color",
        caption: "Accent, transparency, colour theme",
        keywords: "color colour dark mode light accent transparency opacity mica",
        aliases: &["colours", "color", "dark-mode"],
    },
    PageDef {
        key: "themes",
        title: "Themes",
        parent: Some("personalization"),
        icon: "preferences-desktop-theme",
        caption: "Install, create, manage",
        keywords: "gtk theme icons cursor",
        aliases: &[],
    },
    PageDef {
        key: "lockscreen",
        title: "Lock screen",
        parent: Some("personalization"),
        icon: "system-lock-screen",
        caption: "Lock screen images and behaviour",
        keywords: "lock screen greeter",
        aliases: &["lock-screen"],
    },
    PageDef {
        key: "fonts",
        title: "Fonts",
        parent: Some("personalization"),
        icon: "preferences-desktop-font",
        caption: "Install, manage",
        keywords: "font typeface",
        aliases: &[],
    },
    PageDef {
        key: "windowlayout",
        title: "Window Layout",
        parent: Some("personalization"),
        icon: "view-grid",
        caption: "Title bars, snap, gaps, desktops",
        keywords: "tiling snap gaps multitasking titlebar decoration ssd",
        aliases: &["multitasking", "titlebars"],
    },
    PageDef {
        key: "start",
        title: "Start",
        parent: Some("personalization"),
        icon: "start-here",
        caption: "Recent apps and items, folders",
        keywords: "start menu pins recommended",
        aliases: &[],
    },
    PageDef {
        key: "widgets",
        title: "Widgets",
        parent: Some("personalization"),
        icon: "view-grid",
        caption: "Board cards, quiet defaults, third-party drop-ins",
        keywords: "widgets board news weather cards",
        aliases: &["widget"],
    },
    PageDef {
        key: "taskbar",
        title: "Taskbar",
        parent: Some("personalization"),
        icon: "preferences-desktop",
        caption: "Taskbar behaviours, system tray icons",
        keywords: "taskbar panel tray align combine",
        aliases: &[],
    },
    // Apps
    PageDef {
        key: "installed",
        title: "Installed apps",
        parent: Some("apps"),
        icon: "view-grid",
        caption: "Uninstall and manage apps on your PC",
        keywords: "uninstall remove programs",
        aliases: &["installed-apps", "programs"],
    },
    PageDef {
        key: "defaultapps",
        title: "Default apps",
        parent: Some("apps"),
        icon: "preferences-desktop-default-applications",
        caption: "Defaults for file and link types",
        keywords: "default browser email open with",
        aliases: &["default-apps"],
    },
    PageDef {
        key: "startup",
        title: "Startup",
        parent: Some("apps"),
        icon: "system-run",
        caption: "Apps that start when you sign in",
        keywords: "autostart login boot",
        aliases: &["startupapps"],
    },
    // Accounts
    PageDef {
        key: "yourinfo",
        title: "Your info",
        parent: Some("accounts"),
        icon: "avatar-default",
        caption: "Profile photo, name",
        keywords: "picture photo avatar name",
        aliases: &["your-info"],
    },
    PageDef {
        key: "signin",
        title: "Sign-in options",
        parent: Some("accounts"),
        icon: "dialog-password",
        caption: "Password, fingerprint, automatic sign-in",
        keywords: "password fingerprint login",
        aliases: &["signinoptions", "password"],
    },
    PageDef {
        key: "otherusers",
        title: "Other users",
        parent: Some("accounts"),
        icon: "system-users",
        caption: "Add, remove and manage other user accounts",
        keywords: "users family add account",
        aliases: &["otherusers", "family-group"],
    },
    // Time
    PageDef {
        key: "datetime",
        title: "Date & time",
        parent: Some("time"),
        icon: "preferences-system-time",
        caption: "Time zones, automatic clock settings",
        keywords: "clock date timezone ntp",
        aliases: &["dateandtime", "date-time", "clock"],
    },
    PageDef {
        key: "language",
        title: "Language & region",
        parent: Some("time"),
        icon: "preferences-desktop-locale",
        caption: "Language, region, keyboard layouts",
        keywords: "language locale region keyboard",
        aliases: &["regionlanguage", "region"],
    },
    PageDef {
        key: "typing",
        title: "Typing",
        parent: Some("time"),
        icon: "input-keyboard",
        caption: "Key repeat, Caps Lock",
        keywords: "keyboard repeat caps",
        aliases: &[],
    },
    // Gaming
    PageDef {
        key: "gamemode",
        title: "Game Mode",
        parent: Some("gaming"),
        icon: "applications-games",
        caption: "Optimise your PC for play",
        keywords: "game mode gamemode performance",
        aliases: &[],
    },
    PageDef {
        key: "captures",
        title: "Captures",
        parent: Some("gaming"),
        icon: "camera-photo",
        caption: "Save location, screenshot shortcuts",
        keywords: "screenshot capture print screen",
        aliases: &["screenshots"],
    },
    // Accessibility
    PageDef {
        key: "textsize",
        title: "Text size",
        parent: Some("accessibility"),
        icon: "format-text-larger",
        caption: "Text size throughout the desktop and apps",
        keywords: "font size zoom larger",
        aliases: &[],
    },
    PageDef {
        key: "visual",
        title: "Visual effects",
        parent: Some("accessibility"),
        icon: "applications-graphics",
        caption: "Transparency, animation, scrollbars",
        keywords: "animations scrollbars effects transparency opacity",
        aliases: &["visualeffects", "transparency"],
    },
    PageDef {
        key: "pointer",
        title: "Mouse pointer",
        parent: Some("accessibility"),
        icon: "input-mouse",
        caption: "Mouse pointer colour, size",
        keywords: "cursor pointer size",
        aliases: &["cursor"],
    },
    PageDef {
        key: "contrast",
        title: "Contrast themes",
        parent: Some("accessibility"),
        icon: "preferences-desktop-accessibility",
        caption: "Colour themes for low vision",
        keywords: "high contrast low vision",
        aliases: &[],
    },
    PageDef {
        key: "shortcuts",
        title: "Keyboard",
        parent: Some("accessibility"),
        icon: "input-keyboard",
        caption: "Keyboard shortcuts for this desktop",
        keywords: "shortcuts hotkeys",
        aliases: &["keyboard-shortcuts"],
    },
    PageDef {
        key: "tools",
        title: "Productivity tools",
        parent: Some("system"),
        icon: "applications-utilities",
        caption: "Command Palette, FancyZones, Awake, remaps",
        keywords: "powertoys command palette fancyzones awake remap mouse",
        aliases: &["powertoys", "commandpalette", "fancyzones"],
    },
    PageDef {
        key: "notifapps",
        title: "App notifications",
        parent: Some("notifications"),
        icon: "preferences-system-notifications",
        caption: "Per-app banners, sounds, priority",
        keywords: "per-app notification prefs banners sound",
        aliases: &["appnotifications"],
    },
    PageDef {
        key: "trayicons",
        title: "Other system tray icons",
        parent: Some("taskbar"),
        icon: "view-more-symbolic",
        caption: "Show or hide icons in the taskbar corner",
        keywords: "tray icons overflow hidden sni",
        aliases: &["systemtray", "tray"],
    },
    // Privacy
    PageDef {
        key: "security",
        title: "Windows Security",
        parent: Some("privacy"),
        icon: "security-high",
        caption: "Firewall, SELinux, encryption, Secure Boot",
        keywords: "security antivirus selinux",
        aliases: &["windowsdefender"],
    },
    PageDef {
        key: "firewall",
        title: "Firewall & network protection",
        parent: Some("security"),
        icon: "security-high",
        caption: "Firewall status, network profile",
        keywords: "firewall firewalld",
        aliases: &[],
    },
    PageDef {
        key: "camera",
        title: "Camera",
        parent: Some("privacy"),
        icon: "camera-web",
        caption: "Cameras, apps that use the camera",
        keywords: "webcam camera privacy",
        aliases: &[],
    },
    PageDef {
        key: "microphone",
        title: "Microphone",
        parent: Some("privacy"),
        icon: "audio-input-microphone",
        caption: "Microphone access",
        keywords: "mic microphone privacy",
        aliases: &[],
    },
    PageDef {
        key: "location",
        title: "Location",
        parent: Some("privacy"),
        icon: "find-location",
        caption: "Location services",
        keywords: "gps location geoclue",
        aliases: &[],
    },
    PageDef {
        key: "activity",
        title: "Activity history",
        parent: Some("privacy"),
        icon: "document-open-recent",
        caption: "Recent files, clear history",
        keywords: "history recent files",
        aliases: &[],
    },
    // Update
    PageDef {
        key: "updatehistory",
        title: "Update history",
        parent: Some("update"),
        icon: "document-open-recent",
        caption: "Updates, installs and removals",
        keywords: "history dnf installed updates",
        aliases: &["windowsupdate-history"],
    },
    // Advanced
    PageDef {
        key: "mmc",
        title: "Management Console",
        parent: Some("advanced"),
        icon: "preferences-system",
        caption: "Open mde-mmc — primary admin console",
        keywords: "mmc disks services computer management",
        aliases: &["compmgmt"],
    },
    PageDef {
        key: "config",
        title: "Config Editor",
        parent: Some("advanced"),
        icon: "document-properties",
        caption: "Full TOML store (replaces Regedit)",
        keywords: "regedit config toml editor",
        aliases: &["editor", "regedit"],
    },
];

pub fn all_pages() -> Vec<&'static PageDef> {
    CATEGORIES.iter().chain(PAGES.iter()).collect()
}

pub fn get(key: &str) -> Option<&'static PageDef> {
    all_pages().into_iter().find(|p| p.key == key)
}

pub fn resolve(raw: &str) -> &'static str {
    let key = raw
        .trim()
        .trim_start_matches("ms-settings:")
        .to_ascii_lowercase();
    if key.is_empty() {
        return "home";
    }
    if get(&key).is_some() {
        return get(&key).unwrap().key;
    }
    for page in all_pages() {
        if page.aliases.iter().any(|a| a.eq_ignore_ascii_case(&key))
            || page.title.eq_ignore_ascii_case(&key)
        {
            return page.key;
        }
    }
    // Legacy aliases from older mde-control / shell.
    match key.as_str() {
        "personalization" | "colours" => "personalization",
        "system" | "display" => {
            if key == "display" {
                "display"
            } else {
                "system"
            }
        }
        "network" | "network-internet" => "network",
        "bluetooth" => "bluetooth",
        "sound" | "audio" => "sound",
        "clipboard" => "clipboard",
        "accessibility" | "ease" | "easeofaccess" => "accessibility",
        "mmc" | "compmgmt" => "mmc",
        "config" | "editor" | "regedit" => "config",
        "apps" | "applications" => "apps",
        _ => "home",
    }
}

pub fn category_of(key: &str) -> &'static str {
    let mut cur = resolve(key);
    loop {
        let Some(page) = get(cur) else {
            return "home";
        };
        match page.parent {
            Some(parent) => cur = parent,
            None => return page.key,
        }
    }
}

pub fn children(parent: &str) -> Vec<&'static PageDef> {
    PAGES.iter().filter(|p| p.parent == Some(parent)).collect()
}

pub fn breadcrumb(key: &str) -> Vec<&'static PageDef> {
    let mut trail = Vec::new();
    let mut cur = Some(resolve(key));
    while let Some(k) = cur {
        if let Some(page) = get(k) {
            trail.push(page);
            cur = page.parent;
        } else {
            break;
        }
    }
    trail.reverse();
    trail
}

pub fn search_hits(query: &str) -> Vec<&'static PageDef> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    all_pages()
        .into_iter()
        .filter(|p| {
            p.title.to_ascii_lowercase().contains(&q)
                || p.caption.to_ascii_lowercase().contains(&q)
                || p.keywords.to_ascii_lowercase().contains(&q)
                || p.key.contains(&q)
                || p.aliases.iter().any(|a| a.contains(q.as_str()))
        })
        .take(24)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_win11_and_legacy_aliases() {
        assert_eq!(resolve(""), "home");
        assert_eq!(resolve("display"), "display");
        assert_eq!(resolve("ms-settings:colors"), "colors");
        assert_eq!(resolve("sound"), "sound");
        assert_eq!(resolve("audio"), "sound");
        assert_eq!(resolve("bluetooth"), "bluetooth");
        assert_eq!(resolve("network"), "network");
        assert_eq!(resolve("clipboard"), "clipboard");
        assert_eq!(resolve("regedit"), "config");
        assert_eq!(resolve("compmgmt"), "mmc");
        assert_eq!(resolve("easeofaccess"), "accessibility");
        assert_eq!(resolve("windowsupdate"), "update");
        assert_eq!(category_of("sound"), "system");
        assert_eq!(category_of("bluetooth"), "devices");
        assert_eq!(category_of("config"), "advanced");
        assert_eq!(category_of("wifi"), "network");
        assert!(!CATEGORIES.is_empty());
        assert!(get("yourinfo").is_some());
        assert!(search_hits("firewall").iter().any(|p| p.key == "firewall"));
    }
}
