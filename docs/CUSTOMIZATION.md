# 定制化改造 Spec

基于 Pomotroid v1.7.1（Tauri 2 + Svelte 5 + rusqlite）的个人向 fork。

## 总原则

1. **改动尽量放进新文件**。upstream 仍在活跃开发，以后要 rebase，动的既有文件越少冲突越小。
   必须动既有文件时，优先"加参数、加分支"而不是重写函数。
2. **一次一个垂直切片**：DB → query → command → IPC → UI 一条线打通并跑起来，再做下一个。
   不要先把所有表建好再统一写 UI。
3. **保持 upstream 的检查全绿**：`npm run check`、`cargo test`、`cargo clippy -- -D warnings`。
   clippy 零警告是 upstream 的硬要求，别留技术债。
4. 新增的 Rust 模块挂在 `src-tauri/src/` 下独立目录，前端新增组件放 `src/lib/components/<feature>/`。

## 里程碑

| # | 功能 | 依赖 | 状态 |
|---|---|---|---|
| M0 | 环境搭建、原版跑通、改名隔离 | — | ✅ 完成 |
| M1 | 科目（subjects）维度 | M0 | ✅ 完成 |
| M2 | Todolist | M1 | ✅ 完成 |
| M3 | 自定义计时器背景图 | M0 | 待开始 |
| M4 | Google Calendar 同步 | M1 | ✅ 完成（2026-09-26，见 [GOOGLE_CALENDAR.md](GOOGLE_CALENDAR.md)） |
| M5 | 自定义主题、裁剪内置主题 | M1–M4 | 最后做 |

M5 放最后是因为主题要按自己的 UI 设计来定，等功能面稳定了再统一设计，避免返工。

## M5 前置 · 字体排印（2026-09-23）

在配色方案和背景图定下来之前，先做了一版字体/线条的基础打磨，为 M5 铺路。

### 字体方案

新增一套"展示衬线"字体，**只用在固定文案上**（round-label、三个窗口标题、
About 页应用名、设置分组标题），**不用在科目名/任务标题这类用户自己输入的文本上**——
后者本来就该走 Mona Sans → system-ui 的兜底链，硬套精简过字符集的衬线体，
用户一旦打个生僻字就会当场崩字体、句子中间蹦出另一种字体，比不改还难看。

- **中文**：思源宋体（Source Han Serif，Adobe/Google，SIL OFL 1.1）
- **西文**：Source Serif 4（Adobe，SIL OFL 1.1，和思源宋体同一个设计团队做的
  姐妹字重，天生适配，不用赌两个不同来源的字体搭不搭）

**排除的方案**：用户原本提供的截图里有一个"Anthropic Serif"，查证后确认是
Anthropic 自己的品牌定制字体（Chester Jenkins/BSPK 设计，Geist 工作室做的整体
视觉识别），不是开源授权给第三方随便嵌入产品里的——没用。康熙字典体本体已经
因为授权存疑被字体站下架，"润植家康熙字典美化体"这个二创版本虽然免费商用，
但复古书法体只适合大字标题，塞进十几像素的界面小字里会很难认，也没用。

**体积**：两个字体家族都做了精确字符子集化（只保留 `src/messages/*.json`
里实际出现的字符，中文 513 字 + 西文 116 字），四个文件共 ~676KB
（思源宋体三个字重各 ~160KB + Source Serif 变量字体 195KB，
比 Mona Sans 的原版变量字体 530KB 还小）。原始未裁剪的思源宋体单一字重
就有 11MB+，裁剪前完全没法用在桌面应用里。

**以后要加新的固定文案**：如果新字符不在这 513/116 个字符里，会静默 fallback
到 Georgia/Songti SC/serif，不会报错但那个字符看起来就跟周围格格不入。
重新生成子集分两步。

第一步，把 `src/messages/*.json` 里所有字符按"汉字/假名/CJK 标点" vs
"纯西文"分成两桶（CJK 标点和假名要分进汉字桶，不然 Source Serif 里没有对应字形）：

```python
import json, glob

def is_han_family(c):
    o = ord(c)
    return (
        0x4E00 <= o <= 0x9FFF or 0x3400 <= o <= 0x4DBF   # CJK 统一表意文字 + 扩展A
        or 0x3040 <= o <= 0x30FF                          # 平假名+片假名
        or 0x3000 <= o <= 0x303F or 0xFF00 <= o <= 0xFFEF # CJK标点 + 全角形式
        or 0x2018 <= o <= 0x201F                          # 弯引号
    )

chars = set()
for f in glob.glob("src/messages/*.json"):
    for v in json.load(open(f, encoding="utf-8")).values():
        chars.update(v)
chars.update("PomoPipen0123456789:—–…°%+-×÷≈")  # 应用名 + 数字/符号兜底

han = sorted(c for c in chars if is_han_family(c))
latin = sorted(c for c in chars if not is_han_family(c))
open("han.txt", "w", encoding="utf-8").write("".join(han))
open("latin.txt", "w", encoding="utf-8").write("".join(latin))
```

第二步，下载源字体（不进仓库，太大），跑 `fonttools` 子集化：

```bash
pip install "fonttools[woff]" brotli

# 思源宋体：从 adobe-fonts/source-han-serif 的 release 里下 14_SourceHanSerifCN.zip
# （简体中文子集版，比全量版小很多），解出 Regular/SemiBold/Light 三个 otf
python -m fontTools.subset SourceHanSerifCN-Regular.otf \
  --output-file=static/fonts/SourceHanSerifCN-Regular.woff2 --flavor=woff2 \
  --text-file=han.txt --layout-features='*' --glyph-names --no-hinting
# SemiBold / Light 同理，把 --text-file 换成同一个 han.txt 就行

# Source Serif：从 adobe-fonts/source-serif 的 release 里下 Desktop 包，
# 用 VAR/SourceSerif4Variable-Roman.ttf（保留可变字重轴 wght 200-900）
python -m fontTools.subset SourceSerif4Variable-Roman.ttf \
  --output-file=static/fonts/SourceSerif4Variable.woff2 --flavor=woff2 \
  --text-file=latin.txt --unicodes="U+0020-007E,U+00A0-00FF,U+2000-206F" \
  --layout-features='*' --glyph-names --no-hinting
```

两个字体家族的 release 页：
https://github.com/adobe-fonts/source-han-serif/releases 和
https://github.com/adobe-fonts/source-serif/releases。

### 线条

`TimerDial.svelte` 的进度环从 10px 粗实心描边改成 5px，外加一圈 1px 的细
"表圈"轮廓（`<circle>` r=112，用 `--color-separator` 同款的极淡描边）——
原来那个粗圆环是"呆"感的主要来源之一，现在更像手表表盘而不是一块色块。
其余按钮/边框（设置页、任务页那些输入框、卡片边框）这轮没动——
范围上优先保证主计时器窗口（用户 95% 时间看到的界面）先出效果，
其余次要窗口的线条细化留到 M5 配色定下来之后一起弄，避免来回返工。

### 验证方式

这次没有再冒险用原生点击自动化（教训还记着）。截图验证走的是纯只读路径：
`GetWindowRect` + `CopyFromScreen`，全程没有发送任何鼠标点击/按键，
截图后用 PIL 裁剪放大确认"专注"两个字确实带着宋体的顿笔和粗细对比在渲染，
不是继续 fallback 到系统黑体。

## M0 记录 · 改名与数据隔离

应用名 **PomoPipen**，identifier `com.journ.pomopipen`，数据目录
`%APPDATA%\com.journ.pomopipen\pomopipen.db`，全新空库，
和原版 Pomotroid（`com.splode.pomotroid`，419 条 session / 84.7 小时）完全隔离，两者可共存。

改名只动了 10 个文件、21 处。**注意不要误改的地方**：
`settings/defaults.rs` 里的 `theme_light = "Pomotroid Light"` / `theme_dark = "Pomotroid"`
是**主题名**，对应 `static/themes/` 下的 JSON 文件名，不是应用名，改了会导致找不到主题。

### 坑：更新器不能直接删配置

`tauri.conf.json` 原本的 `plugins.updater.endpoints` 指向
`Splode/pomotroid/main/latest.json`。保留它，fork 会去拉上游的安装包。

但**不能直接删掉 `plugins` 块** —— updater 插件在 `Builder::build()` 时就要求配置存在，
不是惰性读取，删了会启动即 panic：

```
PluginInitialization("updater", "Error deserializing 'plugins.updater' ...
invalid type: null, expected struct Config")
```

解法是保留配置块但把 `endpoints` 设为 `[]`。这样插件正常初始化，
`check_update` 命令查无端点，既不会拉上游版本，也不用动插件注册和那两个命令。

### 跑测试一律加 `--test-threads=1`

```bash
cargo test -- --test-threads=1
```

`timer::engine` 里有一批测试是 `std::thread::sleep` + 断言精确 tick 数的，
并行跑、机器有负载时会丢 tick，失败的是哪几个每次都不一样
（见过 3 个 / 2 个 / 1 个三种组合）。单线程跑则 **100 passed 0 failed**。

`engine.rs` 与上游逐字节一致（`git diff upstream/main -- src-tauri/src/timer/engine.rs` 为空），
所以这是上游自带的问题，不是改造引入的。看到这类失败先加 `--test-threads=1` 复核再排查。

还有一个单独的坑：`timer::engine::tests::suspend_and_wake_resume_preserves_position`
断言真实耗时 `>= 3s`（sleep 3 秒后量墙钟时间），机器有负载时哪怕单线程也会因为
调度延迟差一点点就翻车（"got 2"）。连续三次单独重跑是 ok / ok / FAILED。
同样是上游自带、和本次改动无关——这个测试本身对真实耗时的容忍度太紧，不是并行导致的。

### CLAUDE.md 已过时，别照着做

仓库根目录的 `CLAUDE.md` 是重写早期留下的，至少三处与现状不符：

| CLAUDE.md 的说法 | 实际情况 |
|---|---|
| "There are no automated tests in this codebase" | 有 100 个 Rust 测试 |
| "Time: stored in DB as **minutes**" | MIGRATION_2 起已改为秒 |
| 新增 IPC 命令要登记进 `capabilities/default.json` | 不用。该文件只列插件权限，自定义命令由 `core:default` 覆盖 |
| 新增设置项要写 `settings_set` 的 key handler | 不用。`settings_set` 是通用的存任意 key；只有需要副作用（重启服务、刷新托盘等）才加分支 |

## 现状摸底（已确认）

### 数据库

`src-tauri/src/db/migrations.rs`，当前 schema version = 6，四张表：

```sql
sessions(id, started_at, ended_at, round_type, duration_secs, completed)
settings(key, value)          -- 全部按字符串存
custom_themes(id, name, colors)
schema_version(version)
```

`round_type` 有 CHECK 约束，只能是 `work` / `short-break` / `long-break`。
索引：`idx_sessions_started_at`、`idx_sessions_round_type`。

迁移写法是递增常量 + 版本判断，新迁移照抄这个模式，追加 `MIGRATION_7`、`MIGRATION_8`。
**注意**：每个迁移末尾要 `INSERT INTO schema_version VALUES (N);`，
并且 `migrations.rs` 的测试里断言了版本号和表名清单，加表要同步改测试。

### 统计查询

`src-tauri/src/db/queries.rs`，五个查询函数结构高度一致，全都是：

```sql
WHERE round_type = 'work' AND completed = 1
GROUP BY date(started_at, 'unixepoch', 'localtime')
```

- `get_all_time_stats` → `SessionStats`
- `get_daily_stats` → `DailyStats`（含 `by_hour: Vec<u32>` 24 格）
- `get_weekly_stats` → `Vec<DayStat>`（近 7 天）
- `get_heatmap_data` → `Vec<HeatmapEntry>`（全时段，前端按年切片）
- `get_streak` → `StreakInfo`（`compute_streak` 是纯函数，已有 6 个单测）

加科目维度 = 给这几个函数加一个 `subject_id: Option<i64>` 参数，
SQL 里拼一段 `AND (?2 IS NULL OR subject_id = ?2)`。改动机械但要逐个测。

### 命令层

`src-tauri/src/commands.rs`（29 KB，25 个命令），注册在 `src-tauri/src/lib.rs`
的 `invoke_handler![]` 里，按功能分组带注释。新命令追加新分组即可。

前端 IPC 包装统一在 `src/lib/ipc/index.ts`，类型在 `src/lib/types.ts`。

### 前端结构

```
src/routes/+page.svelte          主计时器页
src/routes/settings/+page.svelte 设置页
src/routes/stats/+page.svelte    统计页
src/lib/components/
  Timer.svelte  TimerDial.svelte  TimerDisplay.svelte  TimerFooter.svelte
  stats/DailyView.svelte  WeeklyView.svelte  YearlyView.svelte  ← 热力图在 Yearly
  settings/sections/*.svelte      ← 设置页按 section 分文件，加新 section 不动老文件
```

### 加一个设置项的固定套路（upstream 文档确认）

1. `src-tauri/src/settings/defaults.rs` — 加 key 和默认值
2. `src-tauri/src/settings/mod.rs` — 加到 `Settings` struct
3. `src/lib/types.ts` — 加前端类型
4. `src/lib/components/settings/sections/` — 加 UI 控件

四处都是纯样板，适合交给便宜的模型做。

---

## M1 · 科目维度

### DB（MIGRATION_7）

```sql
CREATE TABLE subjects (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL UNIQUE,
    color      TEXT NOT NULL,              -- hex，用于统计图表和日历事件配色
    archived   INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
    created_at INTEGER NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
);
ALTER TABLE sessions ADD COLUMN subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL;
CREATE INDEX idx_sessions_subject ON sessions(subject_id);
```

**关键决策**：
- `subject_id` 可空。历史 session 和"没选科目"的 session 都是 NULL，统计里归为"未分类"。
  不要造一个 id=0 的"默认科目"，那会污染科目列表。
- 删除科目用 `ON DELETE SET NULL` 而不是级联删除 —— 学习记录不能因为删了科目就消失。
  UI 上优先引导用 `archived` 而不是真删。
- 只有 `round_type='work'` 的 session 才关联科目；休息轮次 `subject_id` 留 NULL。

### 后端

- 新建 `src-tauri/src/subjects/mod.rs`：CRUD + 排序 + 归档。
- `queries.rs` 五个统计函数加 `subject_id: Option<i64>` 过滤参数。
- 新增 `get_subject_breakdown(conn, range)` → 各科目时长占比，给统计页新面板用。
- `insert_session` 加 `subject_id` 参数；调用方在 `timer/` 里，需要把"当前选中科目"
  从设置或内存状态传进来。**当前选中科目存 settings 表**（key: `active_subject_id`），
  这样重启后保持选择，也省一套状态同步。

### 前端

- 计时器页加科目选择器。放在 `TimerFooter.svelte` 附近，注意不要破坏 compact mode 布局。
- 统计页三个 view 加科目筛选下拉；热力图按科目筛选时用该科目的颜色做色阶。
- 新增科目管理界面（设置页新 section，或统计页里的管理入口）。

### 验收

- 切换科目后开始番茄钟，该 session 的 `subject_id` 正确落库
- 统计页按科目筛选，数字和全局视图对得上
- 删除科目后历史记录仍在，归为"未分类"
- 老数据库（version 6）升级后不丢数据

### M1 前端记录（已完成）

**新文件**：`stores/subjects.ts`（共享 store + `watchSubjects()`）、
`SubjectPicker.svelte`（计时器窗口，选当前科目）、
`settings/sections/SubjectsSection.svelte`（新建/改名/换色/排序/归档/删除，全 CRUD）、
`stats/SubjectBreakdown.svelte`（按科目时长列表）。

**改动文件**：`types.ts`/`ipc/index.ts` 加类型和命令；`stores/settings.ts` 加
`active_subject_id` 默认值；`Timer.svelte` 挂载选择器；`settings/+page.svelte` 加
"科目" 分区；`stats/+page.svelte` 加科目筛选 pill 栏；`YearlyView.svelte` 接
`breakdown` 并渲染面板。

**范围取舍**（为了先出基本版）：
- 统计筛选覆盖三个 tab（Today/Week/Heatmap 都吃 `subjectFilter`），但"按科目时长"面板
  只在 All Time tab 显示，且**不受筛选影响**——它永远是"全部科目、全部时间"的参考视图，
  和上面 tab 显示的（可能已筛选的）数据是两回事，别混淆。
- 归档的科目不出现在筛选 pill 栏（只能通过 Settings 里的"显示已归档"管理，不能拿来筛选统计）。
- i18n 只认真翻了 en + zh 两个语言（用户读中文）。其余 6 个语言文件没加新 key ——
  inlang 的 `baseLocale` 是 en，缺失的 key 会自动 fallback 到英文，不会报错或崩，
  只是那 6 个语言看到的是英文。以后要给某个语言补全，直接在对应 `src/messages/<locale>.json`
  加上这 21 个新 key（列表见 `en.json` 里 `nav_subjects` 之后那一段）即可。

**踩到的坑**：
- Svelte 5 的 rename `<input>`用了 HTML `autofocus` 属性一开始没生效——它对"由条件块
  动态插入"的元素不可靠（不同 WebView 实现不一致），改成 `use:` action 手动
  `.focus()`/`.select()` 才稳。TimerSection 的 badge 编辑之所以没这问题，是因为那个
  input 本来就常驻渲染，用户点击时它已经在 DOM 里、天然能拿到焦点。
- `SubjectFilter` 的 serde 序列化：Rust 侧 `#[serde(rename_all = "camelCase")]`
  加在枚举上，unit variant 名字本身也会被按这个规则转（`All` → `"all"`），不是只转
  struct 字段名。踩之前没把握，翻源码确认了 `deserialize_option` 对"key 缺失"和
  "显式 null"都会正确产出 `None`，`Option<T>` 类型的 IPC 参数可以放心不传或传 `undefined`。
- **这台机器上原生桌面 UI 自动化不可靠**：多个窗口（Clash Verge、终端、文件资源管理器）
  会抢占前台/焦点，`SetForegroundWindow` 不保证生效，盲点坐标有点到别的应用的风险。
  之后如果还想验证原生窗口的交互效果，别用 Win32 API 模拟点击——要么让用户自己点，
  要么考虑给 dev 模式开 WebView2 远程调试端口走 CDP。

---

## M2 · Todolist

用户明确要求（2026-09-22）：todolist 按 section 分组，**每个科目自动生成一个 section**，
section 内的任务可以设置时长。据此把原设计从"扁平列表"改成下面这版。

### DB（MIGRATION_8）

Section 不是独立实体，就是科目本身——不建 sections 表，靠 `tasks.subject_id` 分组，
和 `sessions.subject_id`（M1）同一个模式：可空，`ON DELETE SET NULL`，删科目不删任务，
归入"未分类" section。

```sql
CREATE TABLE tasks (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    title        TEXT NOT NULL,
    subject_id   INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    done         INTEGER NOT NULL DEFAULT 0 CHECK(done IN (0,1)),
    est_minutes  INTEGER,                  -- 预估时长（分钟），可空
    created_at   INTEGER NOT NULL,
    completed_at INTEGER,
    sort_order   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_tasks_subject ON tasks(subject_id);

ALTER TABLE sessions ADD COLUMN task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL;
CREATE INDEX idx_sessions_task ON sessions(task_id);
```

**时长用分钟不用番茄数**——"预计 45 分钟"比"预计 1.8 个番茄"直接，且不用跟着
`time_work_secs` 变化重新换算。

**删除 vs 完成**：任务没有"归档"概念（不像科目）。勾完成（`done`）就是待办事项
自身的"完成"语义；真正的 Delete 是从列表里拿掉。删除任务后，之前挂在它上面的
`sessions.task_id` 通过 `ON DELETE SET NULL` 保留，不会丢时长记录。

### 后端

新建 `src-tauri/src/tasks/mod.rs`，结构照抄 `subjects/mod.rs`（校验 + CRUD + reorder），
但拆分成更细的单一职责命令而不是一个大杂烩 `update`，因为 `est_minutes` 需要能被
显式清空（"取消预估"），用嵌套 `Option<Option<T>>` 走 IPC 很别扭，不如各开一个命令：

- `tasks_list(include_done) -> Vec<Task>`（`Task` 带一个查询算出来的 `actual_secs`——
  该任务关联的已完成 work session 时长之和，用于"预计 vs 实际"对比）
- `tasks_create(title, subject_id, est_minutes)`
- `tasks_rename(id, title)`
- `tasks_set_estimate(id, est_minutes: Option<u32>)` —— `None` 就是清空预估
- `tasks_move(id, subject_id: Option<i64>)` —— 换 section，`None` 就是挪去未分类
- `tasks_set_done(id, done)`
- `tasks_delete(id)`
- `tasks_reorder(subject_id, ids)` —— 排序按 section 隔离
- `tasks_set_active(id: Option<i64>)` —— 见下

`active_task_id` 挂进 Settings（同 `active_subject_id` 的模式）。选中一个任务时
**连带把 `active_subject_id` 设成该任务的科目**（两者必须一致，任务从属于科目），
并复用 M1 的"回补当前会话"逻辑（把 `retag_open_work_session` 扩展成同时接受
subject_id 和 task_id 两个可选参数）。

### 前端

**不在计时器窗口塞任务选择器**——那个窗口只有 360px 宽，已经放了科目徽章，
再塞一层任务下拉会很挤。任务选择放在 Todolist 页面本身：每个任务行一个
"设为当前任务"按钮，点了自动带出科目（同时更新 `active_subject_id`）。
计时器窗口的科目徽章顺带在第二行显示当前任务名，给个反馈，不用新开交互面。

新增路由 `src/routes/tasks/+page.svelte`，从 Titlebar 新增一个入口图标打开
（比照 settings/stats 现有的开新窗口模式）。按科目分组渲染：$subjects store
里每个非归档科目固定显示一个 section（哪怕还没有任务，方便直接往里加），
外加一个"未分类" section（只有存在孤儿任务时才显示，避免空占地方）。

### M2 记录（已完成）

**实现时发现的设计问题，和 spec 不一样的地方**：上面写的"未分类 section 只有
存在孤儿任务时才显示"会导致鸡生蛋——没有任何入口能创建一个"不属于任何科目"的
新任务（总得先有个 section 才能往里加）。改成和科目 section 一样**永远显示**，
不管有没有任务，行为统一、不用特判。

**新文件**：`stores/tasks.ts`（同 `stores/subjects.ts` 模式）、
`tasks/TaskRow.svelte`（单条任务：勾选完成、点标题内联改名、点预估时长徽章改数字、
"设为当前"图钉、删除带确认）、`routes/tasks/+page.svelte`（整个 Todolist 窗口，
480×620，从 Titlebar 新图标打开，窗口 chrome 照抄 stats 页那套自定义 titlebar）。

**改动文件**：`types.ts`/`ipc/index.ts` 加 `Task` 类型和 9 个新命令；
`Settings` 加 `active_task_id`；`Titlebar.svelte` 加任务图标按钮（复用现有的
settings/stats 开窗口模式）；`SubjectPicker.svelte` 加第二行显示当前任务名（只读，
点这里不能选任务——选任务在 Todolist 页面本身完成）；`capabilities/default.json`
的 `windows` 数组加 `"tasks"`（否则新窗口拿不到 IPC 权限）。

**后端设计要点**：
- `tasks.subject_id` 和 `sessions.subject_id`（M1）同一套模式：可空、
  `ON DELETE SET NULL`，删科目不删任务，任务落回未分类。
- `est_minutes` 拆成独立命令 `tasks_set_estimate(id, Option<u32>)` 而不是塞进
  一个大杂烩 `tasks_update`——因为它需要能被显式清空成"无预估"，`None` 在
  IPC 层天然表达"清空"，不需要走 `Option<Option<T>>` 那种别扭嵌套。
- `active_task_id` 和 `active_subject_id` 保持"任务选了就带科目，科目单独选
  就清空任务"的单向约束：`tasks_set_active(Some)` 连带把 `active_subject_id`
  设成该任务的科目；`subjects_set_active`（不管传什么）总是清空 `active_task_id`，
  因为那是给"不挑具体任务、只是选个科目"用的粗粒度入口。`subjects_delete` /
  `tasks_delete` 删掉的如果正好是当前选中项，也要连带清対应的 active_* 设置，
  否则新一轮番茄钟会指向一个已经不存在的 id。
- `actual_secs`（任务已花的真实时长）是查询时用 LEFT JOIN 现算的，不落库存储——
  这样"预计 vs 实际"永远和 sessions 表保持一致，不需要额外一套"任务完成时汇总
  写回"的逻辑，也不会因为忘记更新而对不上账。

**测试**：新增 13 个 tasks 模块测试 + 3 个 migration 测试，Rust 测试总数
100 → 114，clippy 干净。`docs/CUSTOMIZATION.md` 里记录的 flaky timer 测试
（`timer::engine` 那几个 sleep-based 的）继续偶发失败，和本次改动无关
（`engine.rs` 一行没动）。

**范围取舍**：Todolist 窗口设的 `resizable: false`（照抄 settings/stats 的既定
模式），没有像主计时器窗口那样另配一套拖拽缩放的手柄——内容溢出交给
`overflow-y: auto` 处理。以后如果任务多到常态性需要更大窗口，再补那套手柄逻辑。

---

## M3 · 自定义背景图

### 设置项（四处样板）

| key | 类型 | 说明 |
|---|---|---|
| `background_image_path` | String | 空字符串表示未设置 |
| `background_opacity` | u8 | 0–100 |
| `background_blur` | u8 | 0–20 px |
| `background_dim` | u8 | 0–100，叠加的暗色遮罩 |

### 实现要点

- 图片通过 Tauri 的 `convertFileSrc` 或读成 base64 注入 CSS。
  优先前者，避免大图进内存。
- **对比度是主要风险**：38 套内置主题的文字色都是按纯色背景配的，铺图后必然有主题不可读。
  所以 dim 遮罩不是可选项而是必需项，默认给一个保守值（比如 40）。
- 图层顺序：背景图 → dim 遮罩 → 现有 UI。不要动 TimerDial 的 SVG 配色逻辑。
- 文件选择用 `@tauri-apps/plugin-dialog`（已是项目依赖）。

---

## M4 · Google Calendar 同步

> **已实现（2026-09-26）**，使用说明和规则见 [GOOGLE_CALENDAR.md](GOOGLE_CALENDAR.md)，代码 `src-tauri/src/gcal/`。
> 和下面最初的计划相比，用户确认改了三处：
> 1. **认证改用 Google 账号登录**（OAuth 桌面应用 + PKCE + 本机回调），不用 Service Account：
>    朋友的试用包也能用自己的账号连接。7 天过期的问题靠把同意屏幕**发布**（不送审）解决；
>    范围只要 `calendar.app.created`，应用只能碰自己建的“PomoPipen 学习记录”日历。
> 2. **调度**：每轮专注完成后约 20 秒（去抖）+ 启动时 + 手动“立即同步”，而不是每天一次。
> 3. **合并规则也用在应用内周历**（`mergeRounds`，src/lib/utils/calendar.ts），两边一致。
>
> 其余（合并粒度、确定性事件 id、14 天滚动窗口、私有标记、只上传科目/时长/任务）按下面的计划做了。
> 标记值是 `src=pomopipen`，事件 id 前缀 `plg` + sha256 前 40 位。

### 最初的认证方案（未采用）

Service Account（不会过期，适合无人值守）：
1. GCP 建项目 → 启用 Calendar API → 建 Service Account → 下载 JSON 密钥
2. Google Calendar 新建专用日历（如"学习记录"）
3. 该日历 → 设置 → 与特定人共享 → 填 service account 邮箱 → 权限"更改活动"

### 事件粒度

同一天 + 同一科目、间隔 ≤ 10 分钟的连续 session 合并成一个事件：

```
标题：📚 高数 · 2h05m
时间：14:02 – 16:07
描述：5 个番茄钟 · 关联任务：第七章习题
颜色：按科目的 color 映射到 Calendar colorId
```

### 幂等

事件 ID 用确定性哈希（Calendar 允许自定义 ID，字符集 base32hex = `a-v0-9`，
sha1 的 hex 输出天然合法）：

```
event_id = "plg" + sha1(subject_id + "|" + block_start_ts)
```

`insert` 撞 409 就 `update`。脚本重复跑、断点重跑都不会产生重复事件。

### 滚动窗口重算

纯增量同步无法反映"本地删了旧记录"。做法：每次重算最近 14 天的所有事件块，
给自己创建的事件打标记 `extendedProperties.private = {src: "pomo-sync"}`，
`events.list` 出窗口内带此标记的事件，本地已不存在的删掉。

**绝不触碰没有这个标记的事件** —— 用户手工建的日程必须安全。

### 调度

Rust 侧 tokio 定时任务，每天固定时间跑一次 + 启动时补跑一次（错过的话）。
上次同步时间存 settings。失败不重试，下次触发自然补上（滚动窗口重算天然容错）。

### 隐私

只上传科目名、时长、番茄数、任务标题。不上传任何窗口标题类信息
（本项目也不采集这些，这点比 PomodoroLogger 干净）。

---

## M5 · 主题

最后做。届时：
- 按自己的 UI 设计规范定制配色
- 裁剪 38 套内置主题，只留常用的几套（`src-tauri/src/themes/mod.rs` 的
  `BUNDLED_JSON` 数组 + 测试里的数量断言要同步改）
- 新增的 UI 元素（科目选择器、任务列表、背景图控件）都要确保在保留的主题下配色正确

---

## 环境备忘

网络环境有坑，换机器或重装时注意：

| 目标 | 走法 |
|---|---|
| GitHub、npm | **必须走代理** `127.0.0.1:7897` |
| 微软（aka.ms、VS 安装器） | **必须绕过代理**，直连可用 |
| Rust 工具链、crates.io | 阿里云镜像直连（清华 / USTC / rsproxy 在此网络下全不可达） |

相关环境变量（已设为用户级）：

```
RUSTUP_HOME       D:\DevTools\rustup
CARGO_HOME        D:\DevTools\cargo
RUSTUP_DIST_SERVER https://mirrors.aliyun.com/rustup
RUSTUP_UPDATE_ROOT https://mirrors.aliyun.com/rustup/rustup
NO_PROXY          mirrors.aliyun.com,localhost,127.0.0.1
```

crates 镜像配在 `D:\DevTools\cargo\config.toml`（source replacement 到阿里云）。
`Cargo.lock` 带每个 crate 的 sha256，cargo 逐个校验，镜像无法在不被发现的情况下篡改。

工具链位置：MSVC 在 `D:\DevTools\VSBuildTools`，
Windows SDK 在 `C:\Program Files (x86)\Windows Kits`（微软不允许改路径）。
