//! Google Calendar sync (fork addition, docs/CUSTOMIZATION.md M4).
//!
//! One way, PomoPipen → Google: completed focus rounds, merged into blocks
//! like the in-app week calendar, become events in a calendar PomoPipen
//! creates in the user's account ("PomoPipen 学习记录").
//!
//! - Sign-in: OAuth for installed apps with the `calendar.app.created` scope,
//!   so the app cannot see or touch any calendar it did not create.
//! - Client: a Google Cloud "Desktop app" client, imported from its JSON in
//!   Settings or embedded at build time (src-tauri/google/client.json).
//! - Sync: rebuild the last 14 local days (the whole history the first time);
//!   create, update and delete only events carrying our private marker.
//!   Runs 20 s after a focus round completes, at startup, and on demand.
//! - Everything lives in `{app_data}/google/connection.json`, outside the
//!   database, so settings resets and database backups never carry the token.
mod api;
mod blocks;
pub mod commands;
mod oauth;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::db::DbState;
use api::{Api, ApiError};
use oauth::{Client, TokenError};

/// Rounds are rebuilt this many local days back on every sync after the first.
const WINDOW_DAYS: u64 = 13;
/// Quiet time after the last trigger before an automatic sync runs.
const DEBOUNCE: Duration = Duration::from_secs(20);

/// What PomoPipen remembers about the Google connection.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Store {
    /// Imported client (otherwise the one embedded at build time).
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
    /// Client the refresh token belongs to; another client needs a new sign-in.
    token_client_id: Option<String>,
    account: Option<String>,
    calendar_id: Option<String>,
    /// Account the calendar was created in.
    calendar_account: Option<String>,
    time_zone: Option<String>,
    auto_sync: Option<bool>,
    /// The whole history has been written once; later syncs only rebuild the window.
    history_synced: bool,
    last_sync: Option<i64>,
    last_error: Option<String>,
    last_result: Option<SyncResult>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct SyncResult {
    pub blocks: u32,
    pub created: u32,
    pub updated: u32,
    pub deleted: u32,
}

/// Sent to the settings window (command results and the `gcal:status` event).
#[derive(Debug, Clone, Serialize)]
pub struct GcalStatus {
    /// Client id in use, if any.
    client_id: Option<String>,
    /// The client came with this build (not imported).
    client_embedded: bool,
    connected: bool,
    account: Option<String>,
    auto_sync: bool,
    syncing: bool,
    connecting: bool,
    last_sync: Option<i64>,
    last_error: Option<String>,
    last_result: Option<SyncResult>,
    /// The proxy Google requests go through right now (`None` = direct).
    proxy: Option<String>,
}

pub struct GcalState {
    dir: PathBuf,
    /// The client and the proxy it was built for; see [`GcalState::http`].
    http: Mutex<(Option<String>, reqwest::Client)>,
    store: Mutex<Store>,
    access: tokio::sync::Mutex<Option<(String, Instant)>>,
    sync_lock: tokio::sync::Mutex<()>,
    syncing: AtomicBool,
    cancel_connect: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    wake: mpsc::UnboundedSender<()>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

impl GcalState {
    fn file(&self) -> PathBuf {
        self.dir.join("connection.json")
    }

    /// The HTTP client for the current system proxy, rebuilt when the proxy
    /// changes (e.g. the proxy app's "system proxy" switched on or to another
    /// port after PomoPipen started).
    fn http(&self) -> reqwest::Client {
        let proxy = api::system_proxy();
        let mut cached = self.http.lock().unwrap();
        if cached.0 != proxy {
            log::info!("[gcal] Google requests now go {}", proxy.as_deref().map_or("direct".to_string(), |p| format!("through {p}")));
            *cached = (proxy.clone(), api::http_client(proxy.as_deref()));
        }
        cached.1.clone()
    }

    fn read(&self) -> Store {
        self.store.lock().unwrap().clone()
    }

    /// Change the store and write it to disk (temp file + rename).
    fn update(&self, change: impl FnOnce(&mut Store)) -> Result<Store, String> {
        let mut store = self.store.lock().unwrap();
        change(&mut store);
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let tmp = self.dir.join("connection.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&*store).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, self.file()).map_err(|e| e.to_string())?;
        Ok(store.clone())
    }

    fn client(&self, store: &Store) -> Option<Client> {
        match (&store.client_id, &store.client_secret) {
            (Some(id), Some(secret)) => Some(Client { id: id.clone(), secret: secret.clone() }),
            _ => oauth::embedded_client(),
        }
    }

    fn connected(&self, store: &Store) -> bool {
        store.refresh_token.is_some()
            && self.client(store).is_some_and(|c| store.token_client_id.as_deref() == Some(c.id.as_str()))
    }

    fn status(&self) -> GcalStatus {
        let store = self.read();
        let client = self.client(&store);
        GcalStatus {
            client_embedded: client.is_some() && store.client_id.is_none(),
            client_id: client.map(|c| c.id),
            connected: self.connected(&store),
            account: store.account.clone(),
            auto_sync: store.auto_sync.unwrap_or(true),
            syncing: self.syncing.load(Ordering::Relaxed),
            connecting: self.cancel_connect.lock().unwrap().is_some(),
            last_sync: store.last_sync,
            last_error: store.last_error.clone(),
            last_result: store.last_result,
            proxy: api::system_proxy(),
        }
    }

    /// A valid access token, refreshing it when it is about to expire.
    async fn access_token(&self) -> Result<String, String> {
        let mut cached = self.access.lock().await;
        if let Some((token, expires)) = cached.as_ref() {
            if *expires > Instant::now() + Duration::from_secs(60) {
                return Ok(token.clone());
            }
        }
        let store = self.read();
        let (Some(client), Some(refresh_token)) = (self.client(&store), store.refresh_token.clone()) else {
            return Err("还没有连接 Google 日历 / Not connected".into());
        };
        match oauth::refresh(&self.http(), &client, &refresh_token).await {
            Ok(tokens) => {
                *cached = Some((tokens.access_token.clone(), Instant::now() + Duration::from_secs(tokens.expires_in)));
                Ok(tokens.access_token)
            }
            Err(TokenError::Revoked) => {
                self.update(|s| s.refresh_token = None)?;
                Err("Google 授权已失效（可能在 Google 账号里移除了访问权限），请重新连接 / The Google sign-in expired; reconnect".into())
            }
            Err(TokenError::Other(e)) => Err(e),
        }
    }
}

fn state(app: &AppHandle) -> Option<Arc<GcalState>> {
    app.try_state::<Arc<GcalState>>().map(|s| Arc::clone(&s))
}

fn emit_status(app: &AppHandle, state: &GcalState) {
    let _ = app.emit("gcal:status", state.status());
}

fn zh_ui(app: &AppHandle) -> bool {
    app.try_state::<DbState>()
        .and_then(|db| db.lock().ok().and_then(|c| crate::settings::load(&c).ok()))
        .is_some_and(|s| s.language.starts_with("zh"))
}

/// Register the state and start the background worker; syncs once at startup.
pub fn init(app: &AppHandle, app_data_dir: PathBuf) {
    let dir = app_data_dir.join("google");
    let store = std::fs::read(dir.join("connection.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let proxy = api::system_proxy();
    log::info!("[gcal] Google requests go {}", proxy.as_deref().map_or("direct".to_string(), |p| format!("through {p}")));
    let http = Mutex::new((proxy.clone(), api::http_client(proxy.as_deref())));
    let (wake, rx) = mpsc::unbounded_channel();
    let state = Arc::new(GcalState {
        dir,
        http,
        store: Mutex::new(store),
        access: tokio::sync::Mutex::new(None),
        sync_lock: tokio::sync::Mutex::new(()),
        syncing: AtomicBool::new(false),
        cancel_connect: Mutex::new(None),
        wake,
    });
    app.manage(Arc::clone(&state));
    let handle = app.clone();
    tauri::async_runtime::spawn(worker(handle, rx));
    schedule_sync(app);
}

/// Ask for an automatic sync soon (after a completed focus round, at startup).
pub fn schedule_sync(app: &AppHandle) {
    if let Some(state) = state(app) {
        let _ = state.wake.send(());
    }
}

/// Focus records were added, moved or deleted by hand (week calendar). A sync
/// normally reconciles only the last WINDOW_DAYS, so an edit before that asks
/// for one full pass, which also removes the event of a record moved away.
pub fn sessions_edited(app: &AppHandle, earliest_start: i64) {
    if let Some(state) = state(app) {
        if earliest_start < blocks::local_midnight_days_ago(WINDOW_DAYS) && state.read().history_synced {
            if let Err(e) = state.update(|s| s.history_synced = false) {
                log::warn!("[gcal] could not mark a full resync: {e}");
            }
        }
    }
    schedule_sync(app);
}

async fn worker(app: AppHandle, mut rx: mpsc::UnboundedReceiver<()>) {
    while rx.recv().await.is_some() {
        // Wait for a quiet moment so a burst of triggers becomes one sync.
        loop {
            match tokio::time::timeout(DEBOUNCE, rx.recv()).await {
                Ok(Some(())) => continue,
                Ok(None) => return,
                Err(_) => break,
            }
        }
        let Some(state) = state(&app) else { continue };
        let store = state.read();
        if store.auto_sync.unwrap_or(true) && state.connected(&store) {
            if let Err(e) = run_sync(&app, &state).await {
                log::warn!("[gcal] automatic sync failed: {e}");
            }
        }
    }
}

/// Sync now, one at a time; records the outcome and tells the windows.
async fn run_sync(app: &AppHandle, state: &GcalState) -> Result<SyncResult, String> {
    let _guard = state.sync_lock.lock().await;
    state.syncing.store(true, Ordering::Relaxed);
    emit_status(app, state);
    let result = sync_once(app, state).await;
    state.syncing.store(false, Ordering::Relaxed);
    let saved = match &result {
        Ok(r) => {
            log::info!("[gcal] synced: {} blocks, +{} ~{} -{}", r.blocks, r.created, r.updated, r.deleted);
            state.update(|s| {
                s.last_sync = Some(unix_now());
                s.last_error = None;
                s.last_result = Some(*r);
            })
        }
        Err(e) => {
            log::warn!("[gcal] sync failed: {e}");
            state.update(|s| s.last_error = Some(e.clone()))
        }
    };
    if let Err(e) = saved {
        log::warn!("[gcal] could not save sync state: {e}");
    }
    emit_status(app, state);
    result
}

fn calendar_texts(zh: bool) -> (&'static str, &'static str) {
    if zh {
        ("PomoPipen 学习记录", "PomoPipen 自动写入的专注记录。只有 PomoPipen 会改这个日历里的事件。")
    } else {
        ("PomoPipen study log", "Focus sessions written by PomoPipen. Only PomoPipen changes the events here.")
    }
}

async fn create_calendar(state: &GcalState, token: &str, zh: bool) -> Result<String, String> {
    let (summary, description) = calendar_texts(zh);
    let tz = state.read().time_zone.unwrap_or_default();
    let http = state.http();
    let id = Api { http: &http, token }.create_calendar(summary, description, &tz).await.map_err(|e| e.message())?;
    let account = state.read().account;
    state.update(|s| {
        s.calendar_id = Some(id.clone());
        s.calendar_account = account;
        s.history_synced = false;
    })?;
    log::info!("[gcal] created calendar");
    Ok(id)
}

async fn sync_once(app: &AppHandle, state: &GcalState) -> Result<SyncResult, String> {
    let zh = zh_ui(app);
    let mut token = state.access_token().await?;
    let mut calendar = match state.read().calendar_id {
        Some(id) => id,
        None => create_calendar(state, &token, zh).await?,
    };
    let mut recreated = false;
    loop {
        let since = if state.read().history_synced { blocks::local_midnight_days_ago(WINDOW_DAYS) } else { 0 };
        let rounds = {
            let db = app.state::<DbState>();
            let conn = db.lock().map_err(|e| e.to_string())?;
            blocks::load_rounds(&conn, since).map_err(|e| e.to_string())?
        };
        let wanted: HashMap<String, serde_json::Value> = blocks::merge(&rounds)
            .iter()
            .map(|b| (blocks::event_id(b), blocks::event_body(b, zh)))
            .collect();
        match apply(state, &token, &calendar, since, &wanted).await {
            Ok(result) => {
                state.update(|s| s.history_synced = true)?;
                return Ok(result);
            }
            // Deleted in Google Calendar: make a fresh one once and write everything again.
            Err(ApiError::NotFound) if !recreated => {
                recreated = true;
                calendar = create_calendar(state, &token, zh).await?;
            }
            // Token revoked or expired early: refresh once.
            Err(ApiError::Unauthorized) if !recreated => {
                recreated = true;
                *state.access.lock().await = None;
                token = state.access_token().await?;
            }
            Err(e) => return Err(e.message()),
        }
    }
}

/// Make the marked events from `since` on match `wanted`.
async fn apply(
    state: &GcalState,
    token: &str,
    calendar: &str,
    since: i64,
    wanted: &HashMap<String, serde_json::Value>,
) -> Result<SyncResult, ApiError> {
    let http = state.http();
    let api = Api { http: &http, token };
    // A missing event must not read as a missing calendar (which recreates it).
    let event_err = |e: ApiError| match e {
        ApiError::NotFound => ApiError::Other("Google: event not found".into()),
        e => e,
    };
    let remote: HashMap<String, serde_json::Value> = api
        .list_events(calendar, &blocks::rfc3339(since))
        .await?
        .into_iter()
        .filter(blocks::is_ours)
        .filter_map(|e| Some((e.get("id")?.as_str()?.to_string(), e)))
        .collect();
    let mut result = SyncResult { blocks: wanted.len() as u32, ..Default::default() };
    for (id, event) in wanted {
        match remote.get(id) {
            Some(existing) if blocks::matches(existing, event) => {}
            Some(_) => {
                api.update_event(calendar, id, event).await.map_err(event_err)?;
                result.updated += 1;
            }
            None => match api.insert_event(calendar, event).await {
                Ok(()) => result.created += 1,
                // The id exists already (e.g. deleted by hand): bring it back.
                Err(ApiError::Conflict) => {
                    api.update_event(calendar, id, event).await.map_err(event_err)?;
                    result.updated += 1;
                }
                Err(e) => return Err(e),
            },
        }
    }
    // Events inside the window whose block is gone locally (records deleted,
    // subject changed, rounds merged). Earlier events are left alone.
    for (id, event) in &remote {
        if !wanted.contains_key(id) && blocks::event_start(event).is_some_and(|start| start >= since) {
            api.delete_event(calendar, id).await?;
            result.deleted += 1;
        }
    }
    Ok(result)
}
