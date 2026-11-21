use crate::place::{DiscoveredHost, Protocol};
use anyhow::Result;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

const WSD_ADDR: &str = "239.255.255.250:3702";

/// Best-effort WS-Discovery Probe for Network/SMB-capable devices.
pub fn probe() -> Result<Vec<DiscoveredHost>> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_read_timeout(Some(Duration::from_millis(800)))?;
    sock.set_broadcast(true)?;

    let uuid = uuid::Uuid::new_v4();
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
 xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
 xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery">
 <soap:Header>
  <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>
  <wsa:MessageID>uuid:{uuid}</wsa:MessageID>
  <wsa:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</wsa:To>
 </soap:Header>
 <soap:Body><wsd:Probe/></soap:Body>
</soap:Envelope>"#
    );

    let _ = sock.send_to(body.as_bytes(), WSD_ADDR);

    let mut out = Vec::new();
    let mut buf = [0u8; 65535];
    let deadline = Instant::now() + Duration::from_millis(900);
    while Instant::now() < deadline {
        match sock.recv_from(&mut buf) {
            Ok((n, addr)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                let host = addr.ip().to_string();
                // Prefer computer name from XML if present
                let name = extract_tag(&text, "Name")
                    .or_else(|| extract_tag(&text, "types"))
                    .unwrap_or_else(|| host.clone());
                out.push(DiscoveredHost {
                    name,
                    host,
                    protocol: Protocol::Smb,
                    port: Some(445),
                    path: String::new(),
                    url: String::new(),
                    reachable: true,
                    source: "ws-discovery".into(),
                    last_seen: 0,
                });
            }
            Err(_) => break,
        }
    }
    Ok(out)
}

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let start = format!("<{tag}");
    let end = format!("</{tag}>");
    let i = xml.find(&start)?;
    let after = &xml[i + start.len()..];
    let gt = after.find('>')?;
    let rest = &after[gt + 1..];
    let j = rest.find(&end)?;
    let val = rest[..j].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}
