//! The few Google Calendar API v3 calls the sync needs.
use serde_json::{json, Value};

const BASE: &str = "https://www.googleapis.com/calendar/v3";

#[derive(Debug)]
pub enum ApiError {
    /// The access token was rejected (expired early or revoked).
    Unauthorized,
    /// The calendar (or event) no longer exists.
    NotFound,
    /// An event with this id already exists (possibly deleted): update it instead.
    Conflict,
    Other(String),
}

impl ApiError {
    pub fn message(&self) -> String {
        match self {
            ApiError::Unauthorized => "Google 拒绝了授权，请重新连接 / Google rejected the sign-in; reconnect".into(),
            ApiError::NotFound => "日历不存在 / calendar not found".into(),
            ApiError::Conflict => "事件已存在 / event exists".into(),
            ApiError::Other(m) => m.clone(),
        }
    }
}

/// A network failure in words that point at the usual cause here: Google needs
/// the proxy. Also logs the full error chain, which the message leaves out.
pub fn network_message(e: &reqwest::Error) -> String {
    log::warn!("[gcal] network error: {e:?}");
    if e.is_timeout() || e.is_connect() {
        match system_proxy() {
            Some(proxy) => format!(
                "连不上 Google：经代理 {proxy} 也没连上，请确认代理节点现在能打开 Google / Cannot reach Google through the proxy {proxy}; check that it can open Google right now"
            ),
            None => "连不上 Google：没有检测到系统代理，请在代理软件里打开“系统代理” / Cannot reach Google: no system proxy is set; turn on your proxy app's system proxy".into(),
        }
    } else {
        format!("网络错误 / network error: {e}")
    }
}

/// The proxy Google requests go through: `HTTPS_PROXY` / `ALL_PROXY` when set
/// (a terminal launch), otherwise the Windows system proxy that Clash and
/// similar apps switch on. Read fresh each time, so turning the proxy on after
/// PomoPipen started still works. `None` = connect directly.
pub fn system_proxy() -> Option<String> {
    for name in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"] {
        if let Ok(value) = std::env::var(name) {
            if !value.trim().is_empty() {
                return Some(with_scheme(value.trim()));
            }
        }
    }
    windows_proxy()
}

#[cfg(windows)]
fn windows_proxy() -> Option<String> {
    let settings = windows_registry::CURRENT_USER
        .open(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;
    if settings.get_u32("ProxyEnable").unwrap_or(0) == 0 {
        return None;
    }
    parse_proxy_server(&settings.get_string("ProxyServer").ok()?)
}

#[cfg(not(windows))]
fn windows_proxy() -> Option<String> {
    None
}

/// Windows stores `ProxyServer` either as one `host:port` for every protocol or
/// per protocol (`http=host:port;https=host:port;socks=host:port`). reqwest's
/// own system-proxy support only understands the first form.
fn parse_proxy_server(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if !value.contains('=') {
        return Some(with_scheme(value));
    }
    let entries: Vec<(&str, &str)> = value
        .split(';')
        .filter_map(|entry| entry.split_once('='))
        .map(|(scheme, server)| (scheme.trim(), server.trim()))
        .filter(|(_, server)| !server.is_empty())
        .collect();
    ["https", "http"]
        .iter()
        .find_map(|wanted| entries.iter().find(|(scheme, _)| scheme.eq_ignore_ascii_case(wanted)))
        .map(|(_, server)| with_scheme(server))
}

fn with_scheme(proxy: &str) -> String {
    if proxy.contains("://") {
        proxy.to_string()
    } else {
        format!("http://{proxy}")
    }
}

/// The HTTP client for Google, through `proxy` (from [`system_proxy`]) or
/// direct. reqwest is built without its own TLS crypto provider (the updater
/// brings ring and installs it only when it runs), so install ring as the
/// process default first; building would panic otherwise.
pub fn http_client(proxy: Option<&str>) -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .no_proxy();
    if let Some(url) = proxy {
        match reqwest::Proxy::all(url) {
            Ok(proxy) => builder = builder.proxy(proxy),
            Err(e) => log::warn!("[gcal] ignoring unusable proxy {url}: {e}"),
        }
    }
    builder.build().expect("HTTP client with the ring provider")
}

/// Tries for a request that could not reach Google at all (e.g. the proxy's
/// node timing out for a moment). Such a request was never sent, so sending it
/// again is safe; a timeout after sending is not retried.
const CONNECT_ATTEMPTS: u64 = 3;
/// Pause before the n-th retry: n × this.
#[cfg(not(test))]
const RETRY_PAUSE: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(test)]
const RETRY_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);

/// Send `request`, retrying connection failures with a short pause.
pub async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response, reqwest::Error> {
    for attempt in 1..CONNECT_ATTEMPTS {
        let Some(copy) = request.try_clone() else { break };
        match copy.send().await {
            Err(e) if e.is_connect() => {
                log::info!("[gcal] connection failed (try {attempt} of {CONNECT_ATTEMPTS}), retrying: {e}");
                tokio::time::sleep(RETRY_PAUSE * attempt as u32).await;
            }
            result => return result,
        }
    }
    request.send().await
}

pub struct Api<'a> {
    pub http: &'a reqwest::Client,
    pub token: &'a str,
}

fn url(segments: &[&str]) -> url::Url {
    let mut u = url::Url::parse(BASE).expect("valid base url");
    u.path_segments_mut().expect("base has a path").extend(segments);
    u
}

async fn read(response: reqwest::Response) -> Result<Value, ApiError> {
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    match status.as_u16() {
        200..=299 => Ok(body),
        401 => Err(ApiError::Unauthorized),
        404 | 410 => Err(ApiError::NotFound),
        409 => Err(ApiError::Conflict),
        code => Err(ApiError::Other(format!(
            "Google {code}: {}",
            body.pointer("/error/message").and_then(Value::as_str).unwrap_or("request failed")
        ))),
    }
}

impl Api<'_> {
    async fn send(&self, request: reqwest::RequestBuilder) -> Result<Value, ApiError> {
        let response = send(request.bearer_auth(self.token)).await.map_err(|e| ApiError::Other(network_message(&e)))?;
        read(response).await
    }

    /// Create PomoPipen's own calendar; returns its id.
    pub async fn create_calendar(&self, summary: &str, description: &str, time_zone: &str) -> Result<String, ApiError> {
        let mut body = json!({ "summary": summary, "description": description });
        if !time_zone.is_empty() {
            body["timeZone"] = json!(time_zone);
        }
        let created = self.send(self.http.post(url(&["calendars"])).json(&body)).await?;
        created.get("id").and_then(Value::as_str).map(String::from).ok_or_else(|| ApiError::Other("Google returned no calendar id".into()))
    }

    /// Our (marked) events that end after `time_min`, following every page.
    pub async fn list_events(&self, calendar: &str, time_min: &str) -> Result<Vec<Value>, ApiError> {
        let mut events = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let mut query = vec![
                ("privateExtendedProperty", "src=pomopipen".to_string()),
                ("timeMin", time_min.to_string()),
                ("singleEvents", "true".into()),
                ("showDeleted", "false".into()),
                ("maxResults", "2500".into()),
            ];
            if let Some(token) = &page {
                query.push(("pageToken", token.clone()));
            }
            let body = self.send(self.http.get(url(&["calendars", calendar, "events"])).query(&query)).await?;
            if let Some(items) = body.get("items").and_then(Value::as_array) {
                events.extend(items.iter().cloned());
            }
            page = body.get("nextPageToken").and_then(Value::as_str).map(String::from);
            if page.is_none() {
                return Ok(events);
            }
        }
    }

    pub async fn insert_event(&self, calendar: &str, event: &Value) -> Result<(), ApiError> {
        self.send(self.http.post(url(&["calendars", calendar, "events"])).json(event)).await.map(|_| ())
    }

    /// Replace an event; also brings back one that was deleted in Google Calendar.
    pub async fn update_event(&self, calendar: &str, id: &str, event: &Value) -> Result<(), ApiError> {
        let mut body = event.clone();
        body["status"] = json!("confirmed");
        self.send(self.http.put(url(&["calendars", calendar, "events", id])).json(&body)).await.map(|_| ())
    }

    pub async fn delete_event(&self, calendar: &str, id: &str) -> Result<(), ApiError> {
        match self.send(self.http.delete(url(&["calendars", calendar, "events", id]))).await {
            Ok(_) | Err(ApiError::NotFound) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_ids_are_escaped_in_paths() {
        let u = url(&["calendars", "abc@group.calendar.google.com", "events"]);
        assert_eq!(u.as_str(), "https://www.googleapis.com/calendar/v3/calendars/abc@group.calendar.google.com/events");
        let odd = url(&["calendars", "a/b#c", "events"]);
        assert_eq!(odd.path(), "/calendar/v3/calendars/a%2Fb%23c/events");
    }

    #[test]
    fn windows_proxy_server_forms_are_understood() {
        assert_eq!(parse_proxy_server("127.0.0.1:7897").as_deref(), Some("http://127.0.0.1:7897"));
        assert_eq!(parse_proxy_server(" http://proxy.lan:8080 ").as_deref(), Some("http://proxy.lan:8080"));
        assert_eq!(
            parse_proxy_server("http=127.0.0.1:7890;https=127.0.0.1:7891;socks=127.0.0.1:7892").as_deref(),
            Some("http://127.0.0.1:7891")
        );
        assert_eq!(parse_proxy_server("HTTP=127.0.0.1:7890;ftp=x:1").as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(parse_proxy_server("socks=127.0.0.1:7892"), None);
        assert_eq!(parse_proxy_server("https=;"), None);
        assert_eq!(parse_proxy_server(""), None);
    }

    #[test]
    fn clients_build_with_and_without_a_proxy() {
        let _ = http_client(None);
        let _ = http_client(Some("http://127.0.0.1:7897"));
        let _ = http_client(Some("not a proxy"));
    }

    /// A fake proxy that answers every connection with `reply` (or just hangs
    /// up) and reports the first line of each request it saw.
    async fn fake_proxy(reply: &'static [u8]) -> (String, tokio::sync::mpsc::UnboundedReceiver<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (seen, requests) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let _ = seen.send(String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or_default().to_string());
                let _ = socket.write_all(reply).await;
            }
        });
        (format!("http://{addr}"), requests)
    }

    fn drain(requests: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> Vec<String> {
        std::iter::from_fn(|| requests.try_recv().ok()).collect()
    }

    /// Like Clash when its node times out: the HTTPS tunnel is refused. The
    /// request never left, so it is tried three times in all.
    #[tokio::test]
    async fn refused_tunnels_are_retried() {
        let (proxy, mut requests) = fake_proxy(b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\n\r\n").await;
        let http = http_client(Some(&proxy));
        let error = send(http.get("https://oauth2.googleapis.com/token")).await.unwrap_err();
        assert!(error.is_connect(), "{error:?}");
        let seen = drain(&mut requests);
        assert_eq!(seen.len(), CONNECT_ATTEMPTS as usize, "{seen:?}");
        assert!(seen.iter().all(|line| line.starts_with("CONNECT oauth2.googleapis.com:443")), "{seen:?}");
    }

    /// Once a request has been handed over, a failure is not retried: Google
    /// may already have acted on it.
    #[tokio::test]
    async fn sent_requests_are_not_retried() {
        let (proxy, mut requests) = fake_proxy(b"").await;
        let http = http_client(Some(&proxy));
        let error = send(http.post("http://example.test/token").body("x")).await.unwrap_err();
        assert!(!error.is_connect(), "{error:?}");
        assert_eq!(drain(&mut requests).len(), 1);
    }
}
