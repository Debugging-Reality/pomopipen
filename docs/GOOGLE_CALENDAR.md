# Google 日历同步

PomoPipen 把已完成的专注单向写进 Google 日历（PomoPipen → Google）。代码在
`src-tauri/src/gcal/`，设置页在 设置 → 日历同步（`CalendarSyncSection.svelte`）。

## 第一次设置（在 Google Cloud 做一次，约 10 分钟）

1. 打开 <https://console.cloud.google.com/>，新建项目（名字随意，如 PomoPipen）。
2. “API 和服务 → 库”，搜索 **Google Calendar API**，启用。
3. 左侧 **Google Auth Platform**（或“OAuth 同意屏幕”）→ 开始：
   - 应用名称 PomoPipen，用户支持邮箱填自己的；
   - 目标对象选 **外部**；联系邮箱填自己的；同意条款 → 创建。
4. “数据访问” → 添加或移除范围 → 手动输入
   `https://www.googleapis.com/auth/calendar.app.created` → 添加 → 保存。
5. “目标对象” → **发布应用** → 确认。
   *必须发布*：停在“测试”状态时，Google 签发的授权 7 天就过期，同步会悄悄停掉。
   发布但不送审是允许的，代价是登录时会看到一次“Google 尚未验证此应用”，最多 100 个用户。
6. “客户端” → 创建客户端 → 应用类型 **桌面应用** → 名称 PomoPipen → 创建 → **下载 JSON**。

## 在 PomoPipen 里连接

1. 设置 → 日历同步 → “选择 JSON 文件…”，选第 6 步下载的文件。
2. 点“连接 Google 日历”，浏览器打开 Google 登录：
   - 选账号；
   - 出现“Google 尚未验证此应用”→ 点“高级”→“转至 PomoPipen（不安全）”；
   - 勾选“创建辅助日历，并查看、创建、更改和删除这些日历中的活动”→ 继续。
3. 浏览器显示“已连接”后回到 PomoPipen。它会在你的 Google 日历里新建
   “PomoPipen 学习记录”日历，并把全部历史写进去。

之后每完成一轮专注约 20 秒自动同步一次，每次启动也补一次；也可以点“立即同步”。

需要能访问 Google 的网络。PomoPipen 使用 Windows 系统代理（Clash 等开了“系统代理”即可；TUN 模式也行）：
每次联网前自己读注册表里的代理（`gcal::api::system_proxy`，也认 `http=…;https=…` 这种分协议写法；
`HTTPS_PROXY` 环境变量优先），代理中途开关或换端口不用重启。设置 → 日历同步 的“网络”一行显示正在用的代理。
连不上（代理节点一时超时、隧道被拒）会自动再试两次；请求已经发出去之后的失败不重试。

## 给朋友的试用包

把第 6 步的 JSON 放到 `src-tauri/google/client.json`（已被 git 忽略）再发布，
客户端就内置在程序里，朋友直接点“连接 Google 日历”，用自己的账号登录即可，不需要导入文件。

## 规则

- **只碰自己的日历**：授权范围是 `calendar.app.created`，PomoPipen 看不到、也改不了你别的日历。
- **合并**：同一天、同一科目、间隔不超过 10 分钟的连续番茄合并成一个事件（和应用里的周历同一规则）。
  标题“🍅 高等数学 · 2h05m”（专注时长），描述里是番茄数和任务，颜色取最接近科目颜色的 Google 事件色。
- **幂等**：事件 id 由科目和块的开始时间决定，重复同步只会更新，不会重复。
- **窗口**：第一次写全部历史；之后每次重算最近 14 天。本地删掉的记录，14 天内对应的事件会被删掉；更早的不动。
  例外：在周历里手动改了 14 天以前的记录（`gcal::sessions_edited`），下一次同步会重算全部历史。
- **标记**：每个事件带私有属性 `src=pomopipen`，同步只改带这个标记的事件。在 Google 里手动改的内容，下次同步会被覆盖。
- **断开**：撤销授权，但日历和事件留在 Google；用同一账号重新连接会继续写同一个日历。
- **存放**：客户端、授权（refresh token）、日历 id 都在数据目录的 `google/connection.json`，
  不在数据库里，所以“恢复默认设置”和数据库备份都不会带走或弄丢它。

## 排查

- 日志里搜 `[gcal]`（设置 → 关于 → 打开日志文件夹）。
- “连不上 Google”：消息里写着用的是哪个代理。先确认这个代理现在能打开 Google（Clash 的节点超时最常见，
  它的日志 `…\clash-verge-rev\logs\sidecar` 里能看到 `dial … i/o timeout`）；PomoPipen 日志里 `[gcal] network error` 有完整错误。
- “授权已失效”：在 Google 账号里移除过访问权限，或同意屏幕还停在“测试”状态超过 7 天。重新连接即可。
- 在 Google 日历里删掉了“PomoPipen 学习记录”：下次同步会新建一个并重新写入全部历史。
