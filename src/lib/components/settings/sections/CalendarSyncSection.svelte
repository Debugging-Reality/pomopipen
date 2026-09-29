<script lang="ts">
  // Settings → Calendar sync: one-way Google Calendar sync (src-tauri/src/gcal).
  import { onMount } from 'svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { settings } from '$lib/stores/settings';
  import { getLocale } from '$paraglide/runtime.js';
  import {
    gcalStatus, gcalImportClient, gcalConnect, gcalCancelConnect, gcalSyncNow,
    gcalDisconnect, gcalSetAuto, onGcalStatus, openJsonFilePicker,
  } from '$lib/ipc';
  import SettingsToggle from '$lib/components/settings/SettingsToggle.svelte';
  import { notify } from '$lib/stores/toast';
  import type { GcalStatus } from '$lib/types';

  const CONSOLE_URL = 'https://console.cloud.google.com/auth/clients';
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let status = $state<GcalStatus | null>(null);
  let actionError = $state<string | null>(null);
  let showSetup = $state(false);

  onMount(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    gcalStatus().then((s) => { status = s; }).catch((e) => { actionError = String(e); });
    void onGcalStatus((s) => { status = s; }).then((u) => { if (disposed) u(); else stop = u; });
    return () => { disposed = true; stop?.(); };
  });

  const when = (secs: number) => {
    const d = new Date(secs * 1000);
    const today = new Date().toDateString() === d.toDateString();
    const time = new Intl.DateTimeFormat(getLocale(), { hour: '2-digit', minute: '2-digit', hour12: false }).format(d);
    return today ? `${zh ? '今天' : 'today'} ${time}` : `${new Intl.DateTimeFormat(getLocale(), { month: 'short', day: 'numeric' }).format(d)} ${time}`;
  };
  let lastLine = $derived.by(() => {
    if (!status?.last_sync) return zh ? '还没有同步过' : 'Not synced yet';
    const r = status.last_result;
    const detail = r ? (zh ? ` · ${r.blocks} 个时间块（新增 ${r.created}、更新 ${r.updated}、删除 ${r.deleted}）` : ` · ${r.blocks} blocks (${r.created} new, ${r.updated} updated, ${r.deleted} removed)`) : '';
    return `${zh ? '上次同步' : 'Last sync'} ${when(status.last_sync)}${detail}`;
  });
  let clientLabel = $derived.by(() => {
    if (!status?.client_id) return zh ? '未配置' : 'Not set up';
    const id = status.client_id;
    const short = id.length > 26 ? `${id.slice(0, 10)}…${id.slice(id.indexOf('.apps'))}` : id;
    return `${short} · ${status.client_embedded ? (zh ? '随应用内置' : 'built in') : (zh ? '已导入' : 'imported')}`;
  });

  async function run(action: () => Promise<GcalStatus>, done?: string) {
    actionError = null;
    try {
      status = await action();
      if (done) notify(done);
    } catch (e) {
      const message = String(e);
      if (!/已取消|Cancelled/.test(message)) actionError = message;
    }
  }
  async function importClient() {
    const path = await openJsonFilePicker().catch(() => null);
    if (path) await run(() => gcalImportClient(path), zh ? '已导入 Google 客户端' : 'Google client imported');
  }
  const connect = () => run(gcalConnect, zh ? '已连接 Google 日历，正在同步…' : 'Connected, syncing…');
  const syncNow = () => run(gcalSyncNow, zh ? '已同步到 Google 日历' : 'Synced to Google Calendar');
  const disconnect = () => run(gcalDisconnect, zh ? '已断开 Google 日历' : 'Disconnected');
</script>

<div class="s-groups">
  <section class="s-group">
    <h3 class="s-group-title">Google {zh ? '日历' : 'Calendar'}</h3>
    <div class="s-card">
      <div class="s-row">
        <div class="s-text">
          <span class="s-label">{zh ? '账号' : 'Account'}</span>
          {#if !status}
            <span class="s-desc">…</span>
          {:else if status.connecting}
            <span class="s-desc">{zh ? '正在等待浏览器里的授权…' : 'Waiting for you to sign in in the browser…'}</span>
          {:else if status.connected}
            <span class="s-desc"><span class="dot on"></span>{zh ? '已连接' : 'Connected'}{status.account ? ` · ${status.account}` : ''}</span>
          {:else if status.client_id}
            <span class="s-desc"><span class="dot"></span>{zh ? '未连接' : 'Not connected'}</span>
          {:else}
            <span class="s-desc"><span class="dot"></span>{zh ? '先在下面导入 Google 客户端' : 'Import a Google client below first'}</span>
          {/if}
        </div>
        <div class="s-control">
          {#if status?.connecting}
            <button class="s-btn s-btn--ghost" onclick={() => gcalCancelConnect()}>{zh ? '取消' : 'Cancel'}</button>
          {:else if status?.connected}
            <button class="s-btn s-btn--ghost" onclick={disconnect}>{zh ? '断开' : 'Disconnect'}</button>
          {:else}
            <button class="s-btn s-btn--primary" disabled={!status?.client_id} onclick={connect}>{zh ? '连接 Google 日历' : 'Connect Google Calendar'}</button>
          {/if}
        </div>
      </div>
      {#if status}
        <div class="s-row">
          <div class="s-text">
            <span class="s-label">{zh ? '网络' : 'Network'}</span>
            <span class="s-desc mono">
              {#if status.proxy}
                {zh ? `经系统代理 ${status.proxy.replace(/^http:\/\//, '')} 连接 Google` : `Reaches Google through the system proxy ${status.proxy.replace(/^http:\/\//, '')}`}
              {:else}
                {zh ? '直连 Google（没有检测到系统代理）' : 'Connects to Google directly (no system proxy set)'}
              {/if}
            </span>
          </div>
        </div>
      {/if}
      {#if status?.connected}
        <div class="s-row">
          <div class="s-text">
            <span class="s-label">{zh ? '同步' : 'Sync'}</span>
            {#if status.syncing}
              <span class="s-desc">{zh ? '正在同步…' : 'Syncing…'}</span>
            {:else if status.last_error}
              <span class="s-hint error" role="alert">{status.last_error}</span>
            {:else}
              <span class="s-desc">{lastLine}</span>
            {/if}
          </div>
          <div class="s-control">
            <button class="s-btn" disabled={status.syncing} onclick={syncNow}>{zh ? '立即同步' : 'Sync now'}</button>
          </div>
        </div>
        <SettingsToggle
          label={zh ? '自动同步' : 'Sync automatically'}
          description={zh ? '每完成一轮专注，约 20 秒后写入；每次打开 PomoPipen 也会补一次。' : 'About 20 seconds after each focus round, and whenever PomoPipen starts.'}
          checked={status.auto_sync}
          onclick={() => run(() => gcalSetAuto(!status!.auto_sync))}
        />
      {/if}
      {#if actionError}
        <div class="s-row"><span class="s-hint error" role="alert">{actionError}</span></div>
      {/if}
    </div>
    <p class="s-note">
      {#if zh}
        写入你 Google 账号里一个新建的“PomoPipen 学习记录”日历，PomoPipen 只能看到和修改这个日历。连续的同科目番茄合并成一个事件（如“🍅 高等数学 · 2h05m”），描述里有番茄数和任务。只上传科目名、时间和任务标题；删掉本地记录后，最近 14 天内对应的事件也会删掉。需要能访问 Google 的网络：PomoPipen 每次联网都读取当前的系统代理（中途开关或换端口不用重启），连不上时会自动重试。
      {:else}
        Writes to a new “PomoPipen study log” calendar in your Google account; PomoPipen can only see and change that calendar. Consecutive rounds of one subject become one event (e.g. “🍅 Calculus · 2h05m”) listing rounds and tasks. Only subject names, times and task titles are uploaded; records deleted here disappear from the last 14 days there too. Needs a network that reaches Google: PomoPipen reads the current system proxy each time it connects (no restart after switching it) and retries failed connections.
      {/if}
    </p>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{zh ? 'Google 客户端' : 'Google client'}</h3>
    <div class="s-card">
      <div class="s-row">
        <div class="s-text">
          <span class="s-label">{zh ? 'OAuth 客户端（桌面应用）' : 'OAuth client (Desktop app)'}</span>
          <span class="s-desc mono">{clientLabel}</span>
        </div>
        <div class="s-control">
          <button class="s-btn s-btn--ghost" onclick={() => (showSetup = !showSetup)} aria-expanded={showSetup || !status?.client_id}>{zh ? '怎么获取' : 'How to get one'}</button>
          <button class="s-btn" onclick={importClient}>{zh ? '选择 JSON 文件…' : 'Choose JSON file…'}</button>
        </div>
      </div>
      {#if showSetup || (status && !status.client_id)}
        <div class="steps">
          {#if zh}
            <ol>
              <li>打开 Google Cloud 控制台，新建一个项目，在“API 和服务 → 库”里启用 <b>Google Calendar API</b>。</li>
              <li>进入 <b>Google Auth Platform</b>：应用名称填 PomoPipen，目标对象选<b>外部</b>；再在“目标对象”页点<b>发布应用</b>（不发布的话授权 7 天就会失效）。</li>
              <li>“数据访问”里添加范围 <code>…/auth/calendar.app.created</code>。</li>
              <li>“客户端”里创建客户端，类型选<b>桌面应用</b>，创建后下载 JSON。</li>
              <li>回到这里点“选择 JSON 文件…”，再点“连接 Google 日历”。浏览器提示“Google 尚未验证此应用”时，点“高级”→“转至 PomoPipen”，并勾选日历权限。</li>
            </ol>
          {:else}
            <ol>
              <li>In Google Cloud Console, create a project and enable the <b>Google Calendar API</b> (APIs &amp; Services → Library).</li>
              <li>Open <b>Google Auth Platform</b>: app name PomoPipen, audience <b>External</b>; then <b>Publish app</b> on the Audience page (unpublished apps lose access after 7 days).</li>
              <li>Under Data access, add the scope <code>…/auth/calendar.app.created</code>.</li>
              <li>Under Clients, create a client of type <b>Desktop app</b> and download its JSON.</li>
              <li>Choose that JSON here, then Connect. When Google warns the app is unverified, pick Advanced → Go to PomoPipen and allow calendar access.</li>
            </ol>
          {/if}
          <button class="s-btn s-btn--ghost" onclick={() => openUrl(CONSOLE_URL)}>{zh ? '打开 Google Cloud 控制台' : 'Open Google Cloud Console'} ↗</button>
        </div>
      {/if}
    </div>
  </section>
</div>

<style>
  .dot { display: inline-block; width: 8px; height: 8px; margin-right: 6px; border-radius: 50%; background: var(--ui-border-strong); vertical-align: 1px; }
  .dot.on { background: var(--color-short-round, #3a8f5a); }
  .mono { font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
  .s-note { margin-top: 8px; }
  .steps { display: flex; flex-direction: column; align-items: flex-start; gap: 10px; padding: 12px 16px 14px; border-top: 1px solid var(--ui-border); }
  .steps ol { margin: 0; padding-left: 20px; display: grid; gap: 6px; font-size: 12.5px; line-height: 1.6; color: var(--ui-text-muted); }
  .steps b { color: var(--ui-text); font-weight: 650; }
  .steps code { font-size: 11.5px; padding: 1px 5px; border-radius: 5px; background: var(--ui-hover); color: var(--ui-text); }
</style>
