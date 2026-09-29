//! Tauri commands for Settings → Calendar sync. Each returns the new status;
//! background changes arrive as the `gcal:status` event.
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use super::{emit_status, oauth, run_sync, state, GcalStatus};

/// How long to wait for the browser sign-in before giving up.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

fn gcal(app: &AppHandle) -> Result<std::sync::Arc<super::GcalState>, String> {
    state(app).ok_or_else(|| "Google Calendar sync is not available".to_string())
}

#[tauri::command]
pub fn gcal_status(app: AppHandle) -> Result<GcalStatus, String> {
    Ok(gcal(&app)?.status())
}

/// Use the client JSON downloaded from Google Cloud (Desktop app).
#[tauri::command]
pub fn gcal_import_client(app: AppHandle, path: String) -> Result<GcalStatus, String> {
    let state = gcal(&app)?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("无法读取文件 / cannot read file: {e}"))?;
    let client = oauth::parse_client_json(&text)?;
    state.update(|s| {
        s.client_id = Some(client.id);
        s.client_secret = Some(client.secret);
    })?;
    log::info!("[gcal] client imported");
    emit_status(&app, &state);
    Ok(state.status())
}

/// Sign in through the browser, create the calendar if needed, then sync.
#[tauri::command]
pub async fn gcal_connect(app: AppHandle, time_zone: String) -> Result<GcalStatus, String> {
    let state = gcal(&app)?;
    let client = state.client(&state.read()).ok_or("还没有配置 Google 客户端 / No Google client configured")?;
    let pending = oauth::begin(&client).await?;
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    if let Some(previous) = state.cancel_connect.lock().unwrap().replace(cancel_tx) {
        let _ = previous.send(());
    }
    emit_status(&app, &state);
    let finish = |state: &super::GcalState| {
        state.cancel_connect.lock().unwrap().take();
        emit_status(&app, state);
    };
    if let Err(e) = app.opener().open_url(&pending.auth_url, None::<&str>) {
        finish(&state);
        return Err(format!("无法打开浏览器 / cannot open the browser: {e}"));
    }
    let code = tokio::select! {
        code = tokio::time::timeout(SIGN_IN_TIMEOUT, oauth::wait_for_code(&pending.listener, &pending.state)) => match code {
            Ok(code) => code,
            Err(_) => Err("等待浏览器登录超时 / Timed out waiting for the browser".into()),
        },
        _ = cancel_rx => Err("已取消 / Cancelled".into()),
    };
    let code = match code {
        Ok(code) => code,
        Err(e) => {
            finish(&state);
            return Err(e);
        }
    };
    let tokens = match oauth::exchange(&state.http(), &client, &code, &pending.verifier, &pending.redirect_uri).await {
        Ok(tokens) => tokens,
        Err(e) => {
            finish(&state);
            return Err(match e {
                oauth::TokenError::Revoked => "Google 拒绝了登录码，请再试一次 / Google rejected the sign-in code; try again".into(),
                oauth::TokenError::Other(e) => e,
            });
        }
    };
    if !tokens.scope.split(' ').any(|s| s == oauth::CALENDAR_SCOPE) {
        finish(&state);
        return Err("授权时没有勾选日历权限，请重新连接并勾选“创建辅助日历…” / Calendar access was not granted; reconnect and allow it".into());
    }
    let Some(refresh_token) = tokens.refresh_token.clone() else {
        finish(&state);
        return Err("Google 没有返回长期授权，请重试 / Google returned no refresh token; try again".into());
    };
    let account = tokens.id_token.as_deref().and_then(oauth::email_from_id_token);
    let saved = state.update(|s| {
        // Another account cannot use the old calendar: the next sync makes a new one.
        if s.calendar_account.is_some() && s.calendar_account != account {
            s.calendar_id = None;
            s.history_synced = false;
        }
        s.refresh_token = Some(refresh_token);
        s.token_client_id = Some(client.id.clone());
        s.account = account;
        s.time_zone = Some(time_zone);
        s.last_error = None;
    });
    *state.access.lock().await = Some((tokens.access_token, Instant::now() + Duration::from_secs(tokens.expires_in)));
    finish(&state);
    saved?;
    log::info!("[gcal] connected");
    let background = state.clone();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = run_sync(&handle, &background).await;
    });
    Ok(state.status())
}

#[tauri::command]
pub fn gcal_cancel_connect(app: AppHandle) -> Result<(), String> {
    if let Some(cancel) = gcal(&app)?.cancel_connect.lock().unwrap().take() {
        let _ = cancel.send(());
    }
    Ok(())
}

#[tauri::command]
pub async fn gcal_sync_now(app: AppHandle) -> Result<GcalStatus, String> {
    let state = gcal(&app)?;
    run_sync(&app, &state).await?;
    Ok(state.status())
}

/// Forget the sign-in (and revoke it at Google). The calendar and its events
/// stay in Google Calendar; reconnecting the same account reuses them.
#[tauri::command]
pub async fn gcal_disconnect(app: AppHandle) -> Result<GcalStatus, String> {
    let state = gcal(&app)?;
    let _guard = state.sync_lock.lock().await;
    if let Some(token) = state.read().refresh_token {
        oauth::revoke(&state.http(), &token).await;
    }
    *state.access.lock().await = None;
    state.update(|s| {
        s.refresh_token = None;
        s.token_client_id = None;
        s.last_error = None;
    })?;
    log::info!("[gcal] disconnected");
    emit_status(&app, &state);
    Ok(state.status())
}

#[tauri::command]
pub fn gcal_set_auto(app: AppHandle, enabled: bool) -> Result<GcalStatus, String> {
    let state = gcal(&app)?;
    state.update(|s| s.auto_sync = Some(enabled))?;
    if enabled {
        super::schedule_sync(&app);
    }
    emit_status(&app, &state);
    Ok(state.status())
}
