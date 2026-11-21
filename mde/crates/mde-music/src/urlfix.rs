//! Normalize media-server base/stream URLs so GStreamer and HTTP clients agree.

/// Ensure `http://` / `https://` form. Fixes `http:host` / `https:host` corruption.
pub fn normalize_http_url(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with("https://") || s.starts_with("http://") {
        return s.trim_end_matches('/').to_string();
    }
    if let Some(rest) = s.strip_prefix("https:") {
        return format!("https://{}", rest.trim_start_matches('/'))
            .trim_end_matches('/')
            .to_string();
    }
    if let Some(rest) = s.strip_prefix("http:") {
        return format!("http://{}", rest.trim_start_matches('/'))
            .trim_end_matches('/')
            .to_string();
    }
    if s.starts_with("//") {
        return format!("http:{s}").trim_end_matches('/').to_string();
    }
    format!("http://{s}").trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_missing_slashes() {
        assert_eq!(
            normalize_http_url("http:172.20.0.2:4040"),
            "http://172.20.0.2:4040"
        );
        assert_eq!(
            normalize_http_url("http://172.20.0.2:4040/"),
            "http://172.20.0.2:4040"
        );
        assert_eq!(
            normalize_http_url("https:example.com/path"),
            "https://example.com/path"
        );
    }
}
