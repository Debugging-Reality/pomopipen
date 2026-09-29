//! Google sign-in for installed apps: the browser signs in, Google redirects
//! to a one-shot listener on 127.0.0.1, and the code is exchanged with PKCE.
//! https://developers.google.com/identity/protocols/oauth2/native-app
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
/// Only calendars this app creates (plus the account's e-mail to show who is connected).
pub const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar.app.created";
const SCOPES: &str = "openid email https://www.googleapis.com/auth/calendar.app.created";

/// A Google Cloud OAuth client of type "Desktop app". Google documents that an
/// installed app's client secret is not confidential; it ships with the app.
#[derive(Debug, Clone, PartialEq)]
pub struct Client {
    pub id: String,
    pub secret: String,
}

/// Parse the JSON Google Cloud offers for download next to the client.
pub fn parse_client_json(text: &str) -> Result<Client, String> {
    let value: Value = serde_json::from_str(text).map_err(|_| "不是有效的 JSON 文件 / not a JSON file".to_string())?;
    let Some(installed) = value.get("installed") else {
        return Err(if value.get("web").is_some() {
            "这是“Web 应用”客户端，请在 Google Cloud 新建“桌面应用”类型的客户端 / This is a Web client; create a Desktop app client".into()
        } else {
            "没有找到 OAuth 客户端信息 / No OAuth client found in this file".into()
        });
    };
    let field = |k: &str| installed.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    match (field("client_id"), field("client_secret")) {
        (Some(id), Some(secret)) => Ok(Client { id, secret }),
        _ => Err("客户端文件缺少 client_id 或 client_secret / Missing client_id or client_secret".into()),
    }
}

/// Client embedded at build time from src-tauri/google/client.json, if present.
pub fn embedded_client() -> Option<Client> {
    option_env!("POMOPIPEN_GOOGLE_CLIENT").and_then(|text| parse_client_json(text).ok())
}

/// A sign-in in progress: the listener waiting for Google's redirect.
pub struct Pending {
    pub listener: TcpListener,
    pub redirect_uri: String,
    pub verifier: String,
    pub state: String,
    pub auth_url: String,
}

fn random_token(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| format!("random: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

pub async fn begin(client: &Client) -> Result<Pending, String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| format!("无法监听本机端口 / cannot listen locally: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    // 32 random bytes → 43 URL-safe characters, as PKCE asks for.
    let verifier = random_token(32)?;
    let state = random_token(16)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let auth_url = url::Url::parse_with_params(AUTH_URL, &[
        ("client_id", client.id.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", state.as_str()),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ])
    .map_err(|e| e.to_string())?
    .to_string();
    Ok(Pending { listener, redirect_uri, verifier, state, auth_url })
}

const PAGE_OK: &str = "<p style=\"font:18px system-ui;margin:3em;text-align:center\">🍅 PomoPipen 已连接 Google 日历，可以关闭这个页面了。<br><small>Connected to Google Calendar. You can close this tab.</small></p>";
const PAGE_FAIL: &str = "<p style=\"font:18px system-ui;margin:3em;text-align:center\">PomoPipen 没有连接 Google 日历，请回到应用重试。<br><small>Not connected. Go back to PomoPipen and try again.</small></p>";

/// The query string of the request line `GET /?a=b HTTP/1.1`.
fn query_of(request: &str) -> Option<&str> {
    let target = request.lines().next()?.split_whitespace().nth(1)?;
    Some(target.split_once('?').map_or("", |(_, q)| q))
}

/// Serve redirects until one carries our `state`; return its authorization code.
pub async fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String, String> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 8192];
        let mut len = 0;
        while len < buf.len() {
            match stream.read(&mut buf[len..]).await {
                Ok(0) | Err(_) => break,
                Ok(n) => len += n,
            }
            if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&buf[..len]).into_owned();
        let params: Vec<(String, String)> = query_of(&request)
            .map(|q| url::form_urlencoded::parse(q.as_bytes()).into_owned().collect())
            .unwrap_or_default();
        let get = |k: &str| params.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        // Browsers also ask for /favicon.ico and the like; ignore anything that is not ours.
        if get("state").as_deref() != Some(state) {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            continue;
        }
        let (page, result) = match (get("code"), get("error")) {
            (Some(code), _) => (PAGE_OK, Ok(code)),
            (None, Some(e)) if e == "access_denied" => (PAGE_FAIL, Err("已在浏览器中取消授权 / Sign-in was cancelled".to_string())),
            (None, e) => (PAGE_FAIL, Err(format!("Google 登录失败 / sign-in failed: {}", e.unwrap_or_default()))),
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
        return result;
    }
}

#[derive(Debug, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    #[serde(default)]
    pub scope: String,
}

/// Why a token request failed; `Revoked` means the user must sign in again.
#[derive(Debug)]
pub enum TokenError {
    Revoked,
    Other(String),
}

async fn token_request(http: &reqwest::Client, form: &[(&str, &str)]) -> Result<Tokens, TokenError> {
    let response = super::api::send(http.post(TOKEN_URL).form(form)).await.map_err(|e| TokenError::Other(super::api::network_message(&e)))?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return serde_json::from_value(body).map_err(|e| TokenError::Other(e.to_string()));
    }
    match body.get("error").and_then(Value::as_str) {
        Some("invalid_grant") => Err(TokenError::Revoked),
        other => Err(TokenError::Other(format!(
            "Google {status}: {}",
            body.get("error_description").and_then(Value::as_str).or(other).unwrap_or("token request failed")
        ))),
    }
}

pub async fn exchange(http: &reqwest::Client, client: &Client, pending_code: &str, verifier: &str, redirect_uri: &str) -> Result<Tokens, TokenError> {
    token_request(http, &[
        ("code", pending_code),
        ("client_id", &client.id),
        ("client_secret", &client.secret),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier),
    ])
    .await
}

pub async fn refresh(http: &reqwest::Client, client: &Client, refresh_token: &str) -> Result<Tokens, TokenError> {
    token_request(http, &[
        ("client_id", &client.id),
        ("client_secret", &client.secret),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ])
    .await
}

/// Best effort: tell Google to forget the grant.
pub async fn revoke(http: &reqwest::Client, token: &str) {
    let _ = super::api::send(http.post(REVOKE_URL).form(&[("token", token)])).await;
}

/// The e-mail inside the ID token Google just returned over TLS (not re-verified).
pub fn email_from_id_token(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()?.get("email")?.as_str().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_desktop_client_json_and_rejects_others() {
        let desktop = r#"{"installed":{"client_id":"123.apps.googleusercontent.com","project_id":"p","client_secret":"GOCSPX-abc","redirect_uris":["http://localhost"]}}"#;
        assert_eq!(parse_client_json(desktop), Ok(Client { id: "123.apps.googleusercontent.com".into(), secret: "GOCSPX-abc".into() }));
        assert!(parse_client_json(r#"{"web":{"client_id":"x","client_secret":"y"}}"#).unwrap_err().contains("桌面应用"));
        assert!(parse_client_json(r#"{"installed":{"client_id":"x"}}"#).is_err());
        assert!(parse_client_json("nope").is_err());
    }

    #[tokio::test]
    async fn auth_url_uses_pkce_loopback_and_the_narrow_scope() {
        let pending = begin(&Client { id: "cid".into(), secret: "s".into() }).await.unwrap();
        let url = url::Url::parse(&pending.auth_url).unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["redirect_uri"], pending.redirect_uri);
        assert!(pending.redirect_uri.starts_with("http://127.0.0.1:"));
        assert_eq!(q["code_challenge"], URL_SAFE_NO_PAD.encode(Sha256::digest(pending.verifier.as_bytes())));
        assert_eq!(q["code_challenge_method"], "S256");
        assert!(q["scope"].contains(CALENDAR_SCOPE) && !q["scope"].contains("auth/calendar "));
        assert_eq!(pending.verifier.len(), 43);
    }

    #[tokio::test]
    async fn loopback_ignores_strangers_and_returns_the_code() {
        let pending = begin(&Client { id: "cid".into(), secret: "s".into() }).await.unwrap();
        let addr = pending.listener.local_addr().unwrap();
        let state = pending.state.clone();
        let browser = tokio::spawn(async move {
            for path in ["/favicon.ico".to_string(), format!("/?state={state}&code=4%2Fabc&scope=email")] {
                let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
                s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes()).await.unwrap();
                let mut reply = String::new();
                s.read_to_string(&mut reply).await.unwrap();
            }
        });
        assert_eq!(wait_for_code(&pending.listener, &pending.state).await.unwrap(), "4/abc");
        browser.await.unwrap();
    }

    /// Network check (run with `--ignored`): Google must answer a bogus client
    /// with its own error, proving TLS and the system proxy work from Rust.
    #[tokio::test]
    #[ignore]
    async fn google_answers_through_the_system_proxy() {
        let http = super::super::api::http_client(super::super::api::system_proxy().as_deref());
        let bogus = Client { id: "0-invalid.apps.googleusercontent.com".into(), secret: "x".into() };
        match refresh(&http, &bogus, "invalid").await {
            Err(TokenError::Other(e)) => assert!(e.contains("Google 401") || e.contains("Google 400"), "{e}"),
            other => panic!("expected Google's rejection, got {other:?}"),
        }
    }

    #[test]
    fn email_comes_from_the_id_token_payload() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"email":"me@example.com","sub":"1"}"#);
        assert_eq!(email_from_id_token(&format!("h.{payload}.sig")), Some("me@example.com".into()));
        assert_eq!(email_from_id_token("garbage"), None);
    }
}
