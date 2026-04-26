# oh-paa Debug Client — Design Spec

## Overview

为 oh-paa 构建一个 Web 端调试客户端，以直观地观察系统运行状态。以开发者调试为核心用例，预留面向交易用户的扩展能力。

### Target Users

- **Primary**: 开发者 — 调试市场数据管道、K 线标准化、LLM 分析任务的执行链路
- **Future**: 交易用户 — 查看分析报告、管理持仓、接收操作建议

### Scope

MVP 聚焦三大视图：市场数据管道 > K 线图表 > LLM 调用追踪。市场范围：加密货币 + 外汇。

---

## Architecture

### Tech Stack

- **Frontend**: React 19 + Vite 6 + TypeScript
- **Styling**: styled-components（复用 socket-everyday 的 neo-brutalist 设计体系）
- **Charts**: ECharts 5（via echarts-for-react）
- **State**: Zustand（全局状态）+ TanStack React Query（服务端缓存）
- **Realtime**: WebSocket（实时事件推送）
- **Design**: JetBrains Mono 字体、2px 硬边框、0px border-radius、黄色主色调

### Monorepo Integration

在 oh-paa 仓库内新建 `web/` 目录，前后端同仓：

```
oh-paa/
├── crates/
│   ├── pa-core/
│   ├── pa-api/        ← 新增 ws.rs + DebugEvent 模块
│   ├── pa-market/     ← 关键路径插入 event emit
│   ├── pa-orchestrator/
│   └── pa-app/        ← 启动时注入 broadcast channel
├── web/               ← 新增 React 前端
│   ├── src/
│   │   ├── pages/
│   │   ├── components/
│   │   ├── charts/
│   │   ├── ws/
│   │   ├── api/
│   │   └── theme/
│   ├── package.json
│   └── vite.config.ts
├── Cargo.toml
└── config.toml
```

### Backend Changes

在 pa-api 中添加 WebSocket 支持（参考 socket-everyday `se-api/src/ws.rs` 模式）：

- 在 `AppState` 中挂载 `tokio::sync::broadcast::Sender<DebugEvent>`
- 新增 `/ws` 端点，客户端连接后接收实时事件流
- 在关键路径发射事件：K 线摄入、Provider 路由决策、标准化结果、LLM 调用开始/结束、任务状态变更
- 通过 topic/channel 分流（debug events vs future user events）

### Data Mode

- **实时推送**: WebSocket 推送新事件（K 线到达、Provider fallback、任务状态变更等）
- **历史回溯**: REST API 查询特定时段的历史数据
- WebSocket 断连时自动重连（指数退避，max 30s），重连后通过 REST 补全 gap

---

## Global Layout

### AppShell

左侧深色 Sidebar + 右侧内容区，参考 socket-everyday 的 AppShell 模式：

- **Sidebar**（200px，可折叠至 64px）
  - 品牌标识 "oh-paa"（黄色）
  - 导航项：Pipeline / K-Line Charts / LLM Trace
  - 底部：System Health 入口 + WebSocket 连接状态指示器（teal "Connected" / red "Disconnected"）
- **Content Area**
  - 最大宽度 1440px，padding 28-36px
  - 页面标题 + 副标题 + 市场筛选 toggle（crypto / forex / cn-a[disabled in MVP]）

---

## View 1: Pipeline — 市场数据管道

### 路由

`/pipeline`

### 信息层级

Market 分类 → Instrument 卡片（展开/折叠）→ Provider 路由事件流

### 顶部统计行

MetricCard 组件（复用 socket-everyday 模式），左侧 4px 彩色 accent border：

| Card | Accent | Value | Sub |
|------|--------|-------|-----|
| Klines Ingested | teal | 总数 | +N last 5min |
| Provider Routes | blue | 活跃路由数 | N fallback active |
| Normalization | yellow | 成功率 % | success rate |
| Errors (1h) | red | 错误数 | 最近错误来源 |

### 市场分类卡片

按 market（crypto / forex）分组，每组标题行（大写、2px 底部边框）。每个 instrument 一张卡片：

**折叠态（默认）**：
- 左：symbol 名称（加粗 14px）+ instrument 名称（灰色）
- 右：M15/H1/D1 三个 bucket 的完成进度（如 `M15: 96/96`）、当前 Provider 名称 + 状态色点、最近延迟、展开箭头
- 异常标的：卡片边框变为红色，显示 fallback badge

**展开态**：
- Provider 详情：primary/fallback 名称及状态
- Last Ingestion 时间 + 距今时长
- Errors (1h) 计数
- Session Bucket Progress：M15/H1/D1 三行进度条，右侧 `N/N ✓` 或异常数标红
- Recent Events：最近 3-5 条事件（时间戳 + 事件描述）

### 底部全局事件流面板

可折叠面板，展示所有 instrument 的实时事件按时间线排列：

- 每行：时间戳 | 状态色点 | symbol | 事件链（如 `kline_ingested → normalize_ok → canonical_stored`）
- 异常事件高亮：`primary_timeout → fallback_routed` 用黄色，失败用红色
- 最大高度 180px，auto scroll 到最新
- WebSocket 实时推送新事件

---

## View 2: K-Line Charts — K 线图表

### 路由

`/kline`

### Top Bar

- **Instrument 选择器**：下拉框显示当前标的（symbol + name + market badge），旁边 pill 按钮按市场分组快速切换
- **缩放控件**：−/Reset/+ 按钮组
- **Timeframe tabs**：M15 | H1 | D1，黄色底色 = active

### 蜡烛图

使用 ECharts candlestick 组件，不含成交量：

**基础交互**：
- 十字准线跟随鼠标：水平虚线 → 右侧价格轴标签（深色底黄字）；垂直虚线 → 底部时间轴标签（深色底黄字）
- 滚轮缩放、拖拽平移、双击重置
- 底部 dataZoom 滑块，可拖拽左右手柄精确控制可视范围

**缩放/平移边界**：

| Timeframe | 初始加载 | 最大可视 (zoom out) | 最小可视 (zoom in) | 数据深度 |
|-----------|---------|--------------------|--------------------|---------|
| M15 | 48 bars (12h) | 192 bars (2天) | 12 bars (3h) | 7天 (lazy load) |
| H1 | 48 bars (2天) | 120 bars (5天) | 12 bars (12h) | 30天 (lazy load) |
| D1 | 60 bars (2月) | 180 bars (6月) | 15 bars (3周) | 365天 (lazy load) |

懒加载策略：初次加载初始范围数据，平移接近左边界时预取下一批（batch size = 最大可视数），总缓存上限 = 数据深度。

**叠加层**：
- PA Overlay（toggle）：每根 bar 下方彩色方块标记 bar reading（teal=bullish, red=bearish, gray=neutral, yellow=inside/special）
- Key Levels（toggle）：水平虚线标注支撑(teal)/阻力(red)/目标(yellow)，来自 SharedBarAnalysisOutput
- Open Bar：虚线蓝色边框蜡烛，通过 WebSocket 实时更新 OHLC，底部标注 "LIVE"

**Hover Tooltip**（180px 宽，跟随鼠标）：

顶部深色标题区：
- 第一行：bar 时间 + 右侧 timeframe · bar state
- 有 PA 数据时：第二行彩色方块 + 彩色 pattern 名称（如 teal "Bullish Engulfing"），第三行灰色简述
- 无 PA 数据时（open bar）：只有第一行

数据区（白底）：
- Open / Close / High / Low — 全称，单列排列，High 用 teal，Low 用 red
- Change — 绝对值 + 百分比并排，如 `+324.30 (+0.48%)`，涨 teal 跌 red

**点击 bar**：锁定选中（黄色边框高亮），底部面板联动更新。

### 底部面板（双栏）

**左：PA Bar Reading**
- 标题 "PA Bar Reading" + 提示 "click a bar to inspect"
- 点击 bar 后显示：pattern 名称（彩色方块 + 加粗文字）+ bar time/timeframe
- 完整分析小结文字（中文）
- 底部标签行：Structure / Bias / Source

**右：Data Inspector**
- Tab 切换：Canonical | Raw | Aggregated | PA State
- 结构化 key-value 表格（不展示原始 JSON），字段解析后直接呈现
- Provider 用彩色 badge，时间格式化为人类可读
- 底部链接：Copy | Open in LLM Trace →（跨视图联动）

---

## View 3: LLM Trace — LLM 调用追踪

拆分为两个页面。

### Page 1: Trigger List

**路由**: `/llm-trace`

**信息层级**: Instrument（切换）→ Trigger 列表（展开任务链）→ 点击任务进入 Page 2

**左上角 Instrument 切换器**：
- 下拉框（symbol + name + market badge）+ 快速切换 pill 按钮（按市场分组，与 K-Line 视图一致）

**筛选器**：
- Task Type：All | PA State | Bar Analysis | Daily Context
- Status：Succeeded | Running | Failed
- Market toggle：crypto | forex

**统计行**（MetricCard）：
- Triggers（today）| Passed | In Progress | Errors | Avg Latency

**Trigger 列表**：
- 按时间倒序排列
- 每个 trigger = instrument + timeframe + bar_close_time
- 折叠态行内容：
  - 左：timeframe badge + bar time（加粗）+ bar state
  - 右：管线状态缩略图 `● → ● → ●`（PA State → Bar Analysis → Daily Context），圆点颜色表状态 + 总耗时/状态文字

圆点状态语义：
- ● teal (filled) = succeeded
- ● blue (filled) = running
- ● red (filled) = failed（旁注 attempt 数如 `3/3`）
- ◇ gray (dashed border) = N/A（该 trigger 不产生此类任务）
- ◇ gray "skipped" = 上游失败导致未执行

异常 trigger 行：红色边框、粉色背景。

**展开 trigger → 任务链**：

垂直时间线（左侧圆形节点 + 竖线），每个节点对应一个 task 卡片：
- 成功 task：✓ teal 节点 + 白底卡片（task type + status badge + provider/latency/attempts + 一行 output 摘要）
- 失败 task：✗ red 节点 + 红底卡片（+ last error 摘要 + retry decision）
- 级联跳过 task：☠ dark+red border 节点 + 灰色虚线卡片（"upstream failed → not executed"）

点击任一 task 卡片中的 "→ detail" → 路由到 Page 2。

### Page 2: Task Detail

**路由**: `/llm-trace/:instrument_id/:trigger_key/:task_id`

**面包屑导航**: ← {instrument} Triggers / {timeframe} · {bar_time} / {task_type}

**Header**: task type（18px 加粗）+ status badge + provider · latency badge + instrument/timeframe/bar 信息

**多 Attempt 场景**: 顶部 tab 切换，每个 tab 标注 "Attempt N · {latency} · {error_type_or_success}"。成功任务（1 attempt）无 tab。

**四段式结构 + 左侧时间轴**：

左侧贯穿时间轴（48px 宽），四个节点方块连接竖线，节点不可交互：
- `→` 深色方块 = Prompt Input
- `←` 深色方块 = LLM Response
- `✓`/`✗` 彩色方块 = Validation（成功 teal / 失败 red）
- `◎`/`☠` 方块 = Analysis Result / Dead Letter

成功路径轴线颜色：深色。失败路径从 Validation 开始变为红色。

右侧四张卡片，全部数据结构化展示（不展示原始 JSON）：

**Card 1 — Prompt Input**：
- 三栏并列：
  - Task Context：key-value 表格（Instrument / Timeframe / Bar Window / Bar State / Input Hash）
  - K-Line Input：表格（Time / Open / High / Low / Close），target bar 黄色高亮行，显示最近 4 条 + "N more bars" 折叠
  - PA State (Upstream)：key-value（Structure / Bias / Resistance / Support / Source）
- 底部链接：View full prompt text →

**Card 2 — LLM Response**：
- 顶部 meta 行：Provider / Model / Latency / HTTP Status
- Reasoning Chain（collapsible `<details>`，默认折叠）：展开后左侧 3px 灰色 border，渲染 LLM 推理文本
- Output Fields：结构化 key-value 表格，展示所有输出字段
  - 成功时：所有字段正常展示
  - 部分失败时：缺失字段标注 `⚠ MISSING`（红色斜体）

**Card 3 — Validation**：
- 成功：Schema 名称 + "Pass" + 绿色确认文字
- 失败：Error Type / Message（code 标签高亮缺失字段名）/ Retryable / Retry Decision（RetryNow 黄色 / MoveToDeadLetter 红色加粗）

**Card 4 — Analysis Result / Dead Letter**：
- 成功（Analysis Result）：
  - 高亮摘要区（灰底虚线边框）：bar reading 彩色方块 + pattern 名 + 一行小结
  - 下方 meta：Task ID / Schema Version / Created At / Finished At
- 失败（Dead Letter）：
  - 半透明灰底虚线边框
  - Final Error / Attempts / Archived At

---

## Cross-View Navigation

三个视图之间的联动跳转：

| From | Action | To |
|------|--------|----|
| Pipeline instrument 卡片 | 点击 symbol 名 | K-Line 视图（预选该 instrument） |
| K-Line Data Inspector | "Open in LLM Trace →" | LLM Trace Page 2（对应 bar 的分析任务） |
| LLM Trace Page 1 task 卡片 | "→ detail" | LLM Trace Page 2 |
| LLM Trace Page 2 header | "View in K-Line →" | K-Line 视图（定位到对应 bar） |
| LLM Trace Page 2 breadcrumb | "← {instrument} Triggers" | LLM Trace Page 1 |

---

## WebSocket Event Protocol

### DebugEvent Enum

```
DebugEvent:
  KlineIngested       { instrument_id, timeframe, open_time, provider, latency_ms }
  ProviderFallback    { instrument_id, primary_provider, fallback_provider, error }
  NormalizationResult { instrument_id, timeframe, open_time, success, error? }
  TaskStatusChanged   { task_id, instrument_id, task_type, old_status, new_status }
  AttemptCompleted    { task_id, attempt_number, provider, model, latency_ms, success, error? }
  OpenBarUpdate       { instrument_id, timeframe, open, high, low, close }
```

### Connection

- 端点：`/ws`
- 认证：未来可扩展 JWT token via query param
- 重连：指数退避（1s → 2s → 4s → ... → max 30s）
- Gap 检测：重连后通过 REST 补全缺失数据

---

## Design System

严格复用 socket-everyday 的 neo-brutalist 设计体系：

### Colors

| Token | Hex | Usage |
|-------|-----|-------|
| Yellow Primary | #FFDE00 | CTA, active tab, highlights |
| Blue Primary | #6FC2FF | secondary actions, running state, links |
| Teal Accent | #53DBC9 | success, bullish, healthy |
| Red Accent | #FF7169 | danger, bearish, errors |
| Beige BG | #F4EFEA | main background |
| Dark | #383838 | borders, text, sidebar |
| Gray | #818181 | secondary text |

### Typography

- Primary: JetBrains Mono
- Fallback: Inter / PingFang SC / Microsoft YaHei
- All numerical data (prices, latencies, counts) in monospace

### Components

- MetricCard: left 4px accent border, eyebrow label + large value + sub text
- Hard edges: 0px border-radius everywhere
- Borders: 2px solid dark (cards), 1px dashed (dividers), 2px dashed (section separators)
- Staggered micro-animations: 60ms delays on MetricCard appearance

---

## Market Scope

### MVP

| Market | Session Type | Providers |
|--------|-------------|-----------|
| Crypto (BTC/USDT, ETH/USDT, SOL/USDT...) | ContinuousUtc (24/7) | TwelveData (primary), EastMoney (fallback) |
| Forex (EUR/USD, GBP/USD...) | Fx24x5Utc (Sun 22:00 - Fri 22:00) | TwelveData (primary), EastMoney (fallback) |

### Future

| Market | Status |
|--------|--------|
| A-shares (cn-a) | UI 预留 toggle（disabled），后续启用 |

---

## Non-Goals (MVP)

- 用户认证/多用户（MVP 单用户调试）
- 用户分析视图（持仓建议、订阅管理）
- 系统健康仪表盘（Provider 连通性、Worker 状态、队列深度）
- 任务编排仪表盘（Pending/Running/Succeeded/Failed 全局概览）
- 移动端适配优化
- Tauri 桌面打包
