//! StatusNotifierWatcher (`org.kde.StatusNotifierWatcher`).
//! Registered items are queried for title, icon name, and status.
//! Open calls `Activate`. Menu reads `com.canonical.dbusmenu` when the item
//! exposes one, and otherwise calls `ContextMenu` so the item draws it.

use futures_util::StreamExt;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};
use zbus::object_server::SignalEmitter;
use zbus::{connection, interface, message::Header};

#[derive(Clone, Debug)]
pub struct SniEntry {
    pub bus: String,
    pub path: String,
    pub title: String,
    pub icon_name: String,
    pub status: String,
    /// Cached PNG from `IconPixmap` when the item does not ship a theme name.
    pub icon_file: String,
}

#[derive(Clone, Default)]
pub struct SniWatch {
    items: Arc<Mutex<Vec<SniEntry>>>,
}

impl SniWatch {
    pub fn snapshot(&self) -> Vec<SniEntry> {
        self.items.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

struct Watcher {
    items: Arc<Mutex<Vec<SniEntry>>>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &self,
        #[zbus(connection)] conn: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        service: &str,
    ) {
        let sender = header.sender().map(|n| n.to_string());
        let Some((bus, path)) = item_address(service, sender.as_deref()) else {
            return;
        };
        let entry = read_item(conn, &bus, &path).await.unwrap_or(SniEntry {
            bus: bus.clone(),
            path: path.clone(),
            title: bus.clone(),
            icon_name: String::new(),
            status: "Active".into(),
            icon_file: String::new(),
        });
        if let Ok(mut guard) = self.items.lock() {
            if let Some(slot) = guard.iter_mut().find(|e| e.bus == entry.bus && e.path == entry.path) {
                *slot = entry;
            } else {
                guard.push(entry);
            }
        }
        let _ = Self::status_notifier_item_registered(&emitter, service).await;
    }

    fn register_status_notifier_host(&self, _service: &str) {}

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items
            .lock()
            .map(|g| g.iter().map(|e| e.bus.clone()).collect())
            .unwrap_or_default()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// `service` is either an object path, a bus name, or `bus/path`.
pub fn item_address(service: &str, sender: Option<&str>) -> Option<(String, String)> {
    let service = service.trim();
    if service.is_empty() {
        return None;
    }
    if let Some(path) = service.strip_prefix('/') {
        let bus = sender?.to_string();
        return Some((bus, format!("/{path}")));
    }
    if let Some((bus, path)) = service.split_once('/') {
        if bus.is_empty() || path.is_empty() {
            return None;
        }
        return Some((bus.to_string(), format!("/{path}")));
    }
    Some((service.to_string(), "/StatusNotifierItem".into()))
}

async fn read_item(conn: &zbus::Connection, bus: &str, path: &str) -> Option<SniEntry> {
    let proxy = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(bus)
        .ok()?
        .path(path)
        .ok()?
        .build()
        .await
        .ok()?;
    let iface = "org.kde.StatusNotifierItem";
    let title = prop_string(&proxy, iface, "Title").await;
    let id = prop_string(&proxy, iface, "Id").await;
    let icon_name = prop_string(&proxy, iface, "IconName").await.unwrap_or_default();
    let status = prop_string(&proxy, iface, "Status")
        .await
        .unwrap_or_else(|| "Active".into());
    let title = title.or(id).unwrap_or_else(|| bus.to_string());
    let icon_file = match prop_pixmap(&proxy, iface).await {
        Some((width, height, bytes)) => {
            crate::icons::cache_pixmap(width, height, &bytes).unwrap_or_default()
        }
        None => String::new(),
    };
    Some(SniEntry {
        bus: bus.to_string(),
        path: path.to_string(),
        title,
        icon_name,
        status,
        icon_file,
    })
}

/// Pick the `IconPixmap` closest to 24px. Bytes stay ARGB32, network order.
pub fn pick_pixmap(value: &zbus::zvariant::Value<'_>) -> Option<(i32, i32, Vec<u8>)> {
    let zbus::zvariant::Value::Array(pixmaps) = value else {
        return None;
    };
    let mut best: Option<(i32, (i32, i32, Vec<u8>))> = None;
    for item in pixmaps.inner() {
        let zbus::zvariant::Value::Structure(structure) = item else {
            continue;
        };
        let fields = structure.fields();
        if fields.len() < 3 {
            continue;
        }
        let zbus::zvariant::Value::I32(width) = fields[0] else {
            continue;
        };
        let zbus::zvariant::Value::I32(height) = fields[1] else {
            continue;
        };
        let Some(bytes) = bytes_of(&fields[2]) else {
            continue;
        };
        if width <= 0 || height <= 0 || width > 256 || height > 256 {
            continue;
        }
        let needed = (width as usize).saturating_mul(height as usize).saturating_mul(4);
        if bytes.len() < needed {
            continue;
        }
        let score = (width - 24).abs() + (height - 24).abs();
        let replace = match &best {
            None => true,
            Some((best_score, _)) => score < *best_score,
        };
        if replace {
            best = Some((score, (width, height, bytes)));
        }
    }
    best.map(|(_, pixmap)| pixmap)
}

fn bytes_of(value: &zbus::zvariant::Value<'_>) -> Option<Vec<u8>> {
    let zbus::zvariant::Value::Array(array) = value else {
        return None;
    };
    let mut bytes = Vec::with_capacity(array.len());
    for item in array.inner() {
        let zbus::zvariant::Value::U8(byte) = item else {
            return None;
        };
        bytes.push(*byte);
    }
    Some(bytes)
}

async fn prop_pixmap(
    proxy: &zbus::fdo::PropertiesProxy<'_>,
    iface: &str,
) -> Option<(i32, i32, Vec<u8>)> {
    let iface = zbus::names::InterfaceName::try_from(iface).ok()?;
    let value = proxy.get(iface, "IconPixmap").await.ok()?;
    pick_pixmap(&value)
}

async fn prop_string(
    proxy: &zbus::fdo::PropertiesProxy<'_>,
    iface: &str,
    name: &str,
) -> Option<String> {
    let iface = zbus::names::InterfaceName::try_from(iface).ok()?;
    let value = proxy.get(iface, name).await.ok()?;
    match &*value {
        zbus::zvariant::Value::Str(text) => {
            let text = text.as_str().trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        _ => None,
    }
}

async fn refresh_all(conn: &zbus::Connection, items: &Mutex<Vec<SniEntry>>) {
    let snapshot = items.lock().map(|g| g.clone()).unwrap_or_default();
    for current in snapshot {
        let Some(updated) = read_item(conn, &current.bus, &current.path).await else {
            continue;
        };
        if let Ok(mut guard) = items.lock() {
            if let Some(slot) = guard
                .iter_mut()
                .find(|e| e.bus == updated.bus && e.path == updated.path)
            {
                *slot = updated;
            }
        }
    }
}

async fn watch_names(conn: zbus::Connection, items: Arc<Mutex<Vec<SniEntry>>>) {
    let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await else {
        return;
    };
    let Ok(mut names) = proxy.receive_name_owner_changed().await else {
        return;
    };
    loop {
        tokio::select! {
            msg = names.next() => {
                let Some(sig) = msg else { break };
                let Ok(args) = sig.args() else { continue };
                let gone = args
                    .new_owner()
                    .as_ref()
                    .map(|owner| owner.as_str().is_empty())
                    .unwrap_or(true);
                if !gone {
                    continue;
                }
                let name = args.name().to_string();
                if let Ok(mut guard) = items.lock() {
                    guard.retain(|entry| entry.bus != name);
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(3)) => {
                refresh_all(&conn, &items).await;
            }
        }
    }
}

#[zbus::proxy(
    interface = "org.kde.StatusNotifierItem",
    default_service = "org.kde.StatusNotifierItem",
    default_path = "/StatusNotifierItem"
)]
trait StatusNotifierItem {
    fn activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn secondary_activate(&self, x: i32, y: i32) -> zbus::Result<()>;
    fn context_menu(&self, x: i32, y: i32) -> zbus::Result<()>;
}

pub fn request_activate(bus: String, path: String) {
    spawn_item_call(bus, path, TrayCall::Activate);
}

pub fn request_secondary_activate(bus: String, path: String) {
    spawn_item_call(bus, path, TrayCall::Secondary);
}

pub fn request_context_menu(bus: String, path: String) {
    spawn_item_call(bus, path, TrayCall::ContextMenu);
}

enum TrayCall {
    Activate,
    Secondary,
    ContextMenu,
}

#[derive(Clone, Debug)]
pub struct MenuEntry {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub depth: u8,
}

#[derive(Clone, Debug)]
pub struct TrayMenu {
    pub item_id: String,
    pub bus: String,
    pub menu_path: String,
    pub entries: Vec<MenuEntry>,
    pub note: String,
}

#[derive(Clone, Default)]
pub struct MenuBoard {
    current: Arc<Mutex<Option<TrayMenu>>>,
    generation: Arc<Mutex<u64>>,
}

impl MenuBoard {
    pub fn snapshot(&self) -> Option<TrayMenu> {
        self.current.lock().ok().and_then(|guard| guard.clone())
    }

    fn store(&self, generation: u64, menu: TrayMenu) {
        let current = self.generation.lock().ok();
        if current.as_ref().is_none_or(|guard| **guard != generation) {
            return;
        }
        if let Ok(mut guard) = self.current.lock() {
            *guard = Some(menu);
        }
    }

    /// Drop any in-flight menu result and return the generation the next load must match.
    pub fn begin(&self) -> u64 {
        let generation = if let Ok(mut guard) = self.generation.lock() {
            *guard = guard.saturating_add(1);
            *guard
        } else {
            0
        };
        if let Ok(mut guard) = self.current.lock() {
            *guard = None;
        }
        generation
    }
}

/// Read the item's DBus menu on a background thread and publish it on `board`.
pub fn load_menu(
    board: MenuBoard,
    generation: u64,
    item_id: String,
    bus: String,
    item_path: String,
) {
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        let menu = rt.block_on(fetch_menu(item_id, bus, item_path));
        board.store(generation, menu);
    });
}

pub fn click_menu(bus: String, menu_path: String, id: i32) {
    if bus.is_empty() || menu_path.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        rt.block_on(async move {
            let Ok(conn) = zbus::Connection::session().await else {
                return;
            };
            let data = zbus::zvariant::Value::I32(0);
            let _ = conn
                .call_method(
                    Some(bus.as_str()),
                    menu_path.as_str(),
                    Some("com.canonical.dbusmenu"),
                    "Event",
                    &(id, "clicked", &data, 0u32),
                )
                .await;
        });
    });
}

async fn fetch_menu(item_id: String, bus: String, item_path: String) -> TrayMenu {
    let empty = |note: String| TrayMenu {
        item_id: item_id.clone(),
        bus: bus.clone(),
        menu_path: String::new(),
        entries: Vec::new(),
        note,
    };
    let Ok(conn) = zbus::Connection::session().await else {
        return empty("Session bus unavailable".into());
    };
    let Some(menu_path) = menu_object_path(&conn, &bus, &item_path).await else {
        request_context_menu(bus.clone(), item_path);
        return empty("No DBus menu; the app was asked to draw it".into());
    };
    let _ = conn
        .call_method(
            Some(bus.as_str()),
            menu_path.as_str(),
            Some("com.canonical.dbusmenu"),
            "AboutToShow",
            &(0i32,),
        )
        .await;
    let names = ["label", "enabled", "visible", "type"];
    let Ok(reply) = conn
        .call_method(
            Some(bus.as_str()),
            menu_path.as_str(),
            Some("com.canonical.dbusmenu"),
            "GetLayout",
            &(0i32, 2i32, &names),
        )
        .await
    else {
        request_context_menu(bus.clone(), item_path);
        return empty("Menu layout failed; the app was asked to draw it".into());
    };
    let Ok((revision, layout)): Result<(u32, zbus::zvariant::OwnedValue), _> =
        reply.body().deserialize()
    else {
        return empty("Menu layout was unreadable".into());
    };
    let _ = revision;
    let mut entries = Vec::new();
    flatten_menu(&layout, 0, &mut entries);
    entries.truncate(16);
    let note = if entries.is_empty() {
        "Menu has no labels".into()
    } else {
        String::new()
    };
    TrayMenu {
        item_id,
        bus,
        menu_path,
        entries,
        note,
    }
}

async fn menu_object_path(conn: &zbus::Connection, bus: &str, path: &str) -> Option<String> {
    let proxy = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(bus)
        .ok()?
        .path(path)
        .ok()?
        .build()
        .await
        .ok()?;
    let iface = zbus::names::InterfaceName::try_from("org.kde.StatusNotifierItem").ok()?;
    let value = proxy.get(iface, "Menu").await.ok()?;
    match &*value {
        zbus::zvariant::Value::ObjectPath(path) => {
            let path = path.as_str();
            if path.is_empty() || path == "/" {
                None
            } else {
                Some(path.to_string())
            }
        }
        zbus::zvariant::Value::Str(text) => {
            let text = text.as_str();
            if text.starts_with('/') && text != "/" {
                Some(text.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Flatten a DBus menu layout `(ia{sv}av)` into labels. At most a few levels.
pub fn flatten_menu(layout: &zbus::zvariant::Value<'_>, depth: u8, out: &mut Vec<MenuEntry>) {
    if out.len() >= 16 || depth > 3 {
        return;
    }
    let layout = match layout {
        zbus::zvariant::Value::Value(inner) => inner,
        other => other,
    };
    let zbus::zvariant::Value::Structure(structure) = layout else {
        return;
    };
    let fields = structure.fields();
    if fields.len() < 3 {
        return;
    }
    let zbus::zvariant::Value::I32(id) = fields[0] else {
        return;
    };
    let label = dict_string(&fields[1], "label");
    let kind = dict_string(&fields[1], "type");
    let visible = dict_bool(&fields[1], "visible").unwrap_or(true);
    let enabled = dict_bool(&fields[1], "enabled").unwrap_or(true);
    if visible && !label.is_empty() && kind != "separator" {
        out.push(MenuEntry {
            id,
            label,
            enabled,
            depth,
        });
    }
    let children = match &fields[2] {
        zbus::zvariant::Value::Array(array) => array.inner(),
        _ => return,
    };
    for child in children {
        flatten_menu(child, depth.saturating_add(1), out);
    }
}

fn dict_string(value: &zbus::zvariant::Value<'_>, key: &str) -> String {
    let Some(found) = dict_get(value, key) else {
        return String::new();
    };
    value_string(found)
}

fn dict_bool(value: &zbus::zvariant::Value<'_>, key: &str) -> Option<bool> {
    match dict_get(value, key)? {
        zbus::zvariant::Value::Bool(flag) => Some(*flag),
        zbus::zvariant::Value::Value(inner) => match inner.as_ref() {
            zbus::zvariant::Value::Bool(flag) => Some(*flag),
            _ => None,
        },
        _ => None,
    }
}

fn dict_get<'a>(
    value: &'a zbus::zvariant::Value<'a>,
    key: &str,
) -> Option<&'a zbus::zvariant::Value<'a>> {
    let zbus::zvariant::Value::Dict(dict) = value else {
        return None;
    };
    for (name, item) in dict.iter() {
        let zbus::zvariant::Value::Str(text) = name else {
            continue;
        };
        if text.as_str() == key {
            return Some(item);
        }
    }
    None
}

fn value_string(value: &zbus::zvariant::Value<'_>) -> String {
    match value {
        zbus::zvariant::Value::Str(text) => text.as_str().to_string(),
        zbus::zvariant::Value::Value(inner) => value_string(inner),
        _ => String::new(),
    }
}

fn spawn_item_call(bus: String, path: String, call: TrayCall) {
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        rt.block_on(async move {
            let Ok(conn) = zbus::Connection::session().await else {
                return;
            };
            let Ok(builder) = StatusNotifierItemProxy::builder(&conn).destination(bus.as_str()) else {
                return;
            };
            let Ok(builder) = builder.path(path.as_str()) else {
                return;
            };
            let Ok(proxy) = builder.build().await else {
                return;
            };
            let result = match call {
                TrayCall::Activate => proxy.activate(0, 0).await,
                TrayCall::Secondary => proxy.secondary_activate(0, 0).await,
                TrayCall::ContextMenu => proxy.context_menu(0, 0).await,
            };
            if let Err(e) = result {
                warn!("tray item call failed: {e}");
            }
        });
    });
}

pub async fn serve(watch: SniWatch) {
    let iface = Watcher {
        items: watch.items.clone(),
    };
    let built = connection::Builder::session()
        .and_then(|b| b.name("org.kde.StatusNotifierWatcher"))
        .and_then(|b| b.serve_at("/StatusNotifierWatcher", iface));
    match built {
        Ok(builder) => match builder.build().await {
            Ok(conn) => {
                info!("claimed org.kde.StatusNotifierWatcher");
                let items = watch.items.clone();
                let monitor = conn.clone();
                tokio::spawn(async move { watch_names(monitor, items).await });
                std::future::pending::<()>().await;
                drop(conn);
            }
            Err(e) => warn!("SNI watcher connect failed: {e}"),
        },
        Err(e) => warn!("SNI watcher setup failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_forms() {
        assert_eq!(
            item_address("/StatusNotifierItem", Some(":1.8")).as_ref().map(|(b, p)| (b.as_str(), p.as_str())),
            Some((":1.8", "/StatusNotifierItem"))
        );
        assert_eq!(
            item_address("org.example.Tray", None).as_ref().map(|(b, p)| (b.as_str(), p.as_str())),
            Some(("org.example.Tray", "/StatusNotifierItem"))
        );
        assert_eq!(
            item_address("org.example.Tray/org/example/Item", None)
                .as_ref()
                .map(|(b, p)| (b.as_str(), p.as_str())),
            Some(("org.example.Tray", "/org/example/Item"))
        );
        assert!(item_address("/StatusNotifierItem", None).is_none());
    }

    #[test]
    fn pixmap_pick_prefers_size_near_24() {
        use zbus::zvariant::{Array, Structure, Value};
        let tiny = Structure::from((1i32, 1i32, vec![255u8, 1, 2, 3]));
        let mut pixmaps = Array::new(tiny.signature());
        pixmaps
            .append(Value::Structure(tiny))
            .expect("tiny pixmap");
        pixmaps
            .append(Value::Structure(Structure::from((
                22i32,
                22i32,
                vec![0u8; 22 * 22 * 4],
            ))))
            .expect("near pixmap");
        let value = Value::Array(pixmaps);
        let (width, height, bytes) = pick_pixmap(&value).expect("pixmap");
        assert_eq!((width, height), (22, 22));
        assert_eq!(bytes.len(), 22 * 22 * 4);
        assert!(pick_pixmap(&Value::from(0i32)).is_none());
    }

    #[test]
    fn menu_layout_flattens_labels() {
        use zbus::zvariant::{Array, Dict, StructureBuilder, Type, Value};
        fn props(label: &str, enabled: bool) -> Dict<'_, '_> {
            let mut dict = Dict::new(<&str as Type>::SIGNATURE, Value::SIGNATURE);
            dict.append(Value::from("label"), Value::Value(Box::new(Value::from(label))))
                .expect("label");
            dict.append(
                Value::from("enabled"),
                Value::Value(Box::new(Value::Bool(enabled))),
            )
            .expect("enabled");
            dict
        }
        let child = StructureBuilder::new()
            .append_field(Value::I32(7))
            .append_field(Value::Dict(props("Quit", true)))
            .append_field(Value::Array(Array::new(Value::SIGNATURE)))
            .build()
            .expect("child");
        let hidden = StructureBuilder::new()
            .append_field(Value::I32(8))
            .append_field(Value::Dict(props("Nope", false)))
            .append_field(Value::Array(Array::new(Value::SIGNATURE)))
            .build()
            .expect("hidden");
        let mut children = Array::new(Value::SIGNATURE);
        children
            .append(Value::Value(Box::new(Value::Structure(child))))
            .expect("child");
        children
            .append(Value::Value(Box::new(Value::Structure(hidden))))
            .expect("hidden");
        let root = Value::Structure(
            StructureBuilder::new()
                .append_field(Value::I32(0))
                .append_field(Value::Dict(props("App", true)))
                .append_field(Value::Array(children))
                .build()
                .expect("root"),
        );
        let mut entries = Vec::new();
        flatten_menu(&root, 0, &mut entries);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].label, "App");
        assert_eq!(entries[1].label, "Quit");
        assert_eq!(entries[1].id, 7);
        assert_eq!(entries[1].depth, 1);
        assert!(!entries[2].enabled);
        assert_eq!(entries[2].label, "Nope");
    }
}
