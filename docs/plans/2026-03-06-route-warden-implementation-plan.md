# Route Warden Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 构建一个在 macOS 稳定常驻运行、可扩展到 Ubuntu 的 Rust 守护程序，按组探测目标站点/API 并自动切换 Mihomo/Clash 最优节点。

**Architecture:** 单进程守护（Runner）按固定周期执行：读取配置 -> 获取节点/组状态 -> 并发探测 -> 评分排序 -> 防抖判定 -> 切换策略组 -> 写入 SQLite。以“可用性优先、延迟次之”为核心评分原则，避免频繁抖动切换。

**Tech Stack:** Rust 2024, tokio, reqwest, serde/serde_yaml, anyhow/thiserror, tracing/tracing-subscriber, rusqlite, clap, wiremock, tempfile.

---

### Task 1: 项目骨架与依赖基线

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/config/mod.rs`
- Create: `src/controller/mod.rs`
- Create: `src/probe/mod.rs`
- Create: `src/score/mod.rs`
- Create: `src/select/mod.rs`
- Create: `src/store/mod.rs`
- Create: `src/runner/mod.rs`
- Create: `tests/smoke_startup.rs`

**Step 1: Write the failing test**

```rust
// tests/smoke_startup.rs
#[test]
fn app_bootstrap_compiles_and_starts() {
    let version = route_warden::app_version();
    assert!(!version.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test smoke_startup -v`  
Expected: FAIL with unresolved crate items (`app_version` not found)

**Step 3: Write minimal implementation**

```rust
// src/lib.rs
pub fn app_version() -> &'static str { env!("CARGO_PKG_VERSION") }

// src/main.rs
fn main() { println!("route-warden {}", route_warden::app_version()); }
```

并在 `Cargo.toml` 增加上述运行所需基础依赖。

**Step 4: Run test to verify it passes**

Run: `cargo test smoke_startup -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml src tests/smoke_startup.rs
git commit -m "初始化 route-warden 项目骨架\n\n- 增加核心模块目录结构\n- 建立最小可运行入口与库导出\n- 添加启动冒烟测试"
```

### Task 2: 配置加载与校验

**Files:**
- Create: `src/config/types.rs`
- Modify: `src/config/mod.rs`
- Create: `tests/config_load.rs`
- Create: `fixtures/config.valid.yaml`
- Create: `fixtures/config.invalid.yaml`

**Step 1: Write the failing test**

```rust
#[test]
fn load_valid_config() {
    let cfg = route_warden::config::load_from_path("fixtures/config.valid.yaml").unwrap();
    assert_eq!(cfg.interval_sec, 180);
    assert!(cfg.groups.contains_key("GLOBAL_BEST"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test config_load -v`  
Expected: FAIL with missing `config` APIs

**Step 3: Write minimal implementation**

实现 `Config` 结构与 `load_from_path`：

```rust
pub fn load_from_path(path: &str) -> anyhow::Result<Config> {
    let txt = std::fs::read_to_string(path)?;
    let cfg: Config = serde_yaml::from_str(&txt)?;
    cfg.validate()?;
    Ok(cfg)
}
```

校验项至少包括：`interval_sec > 0`、目标组存在、目标 URL 非空。

**Step 4: Run test to verify it passes**

Run: `cargo test config_load -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/config tests/config_load.rs fixtures/config.*.yaml Cargo.toml
git commit -m "实现配置加载与基础校验\n\n- 新增 YAML 配置结构与解析入口\n- 增加关键字段校验逻辑\n- 补充有效/无效配置测试样例"
```

### Task 3: Mihomo 控制器客户端（读取节点与切换组）

**Files:**
- Create: `src/controller/client.rs`
- Modify: `src/controller/mod.rs`
- Create: `tests/controller_client.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn switch_proxy_group_node() {
    // 使用 wiremock 模拟 /proxies 与 PUT /proxies/{group}
    let ok = route_warden::controller::switch_group("http://127.0.0.1:PORT", "GLOBAL", "NodeA").await;
    assert!(ok.is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test controller_client -v`  
Expected: FAIL due to missing async client API

**Step 3: Write minimal implementation**

提供 API：
- `list_proxies()`
- `get_group_members(group)`
- `switch_group(group, node)`

使用 `reqwest` + `Bearer secret`（若配置存在）。

**Step 4: Run test to verify it passes**

Run: `cargo test controller_client -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/controller tests/controller_client.rs Cargo.toml
git commit -m "实现 Mihomo 控制器客户端\n\n- 支持节点/组信息读取\n- 支持策略组切换接口\n- 增加 wiremock 单元测试"
```

### Task 4: 目标探测器与状态判定

**Files:**
- Create: `src/probe/http_probe.rs`
- Modify: `src/probe/mod.rs`
- Create: `tests/probe_status_rules.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn classify_status_codes() {
    use route_warden::probe::classify;
    assert!(classify("BINANCE", 403).is_success);
    assert!(!classify("BINANCE", 429).is_success);
    assert!(classify("OPENAI", 401).is_success);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test probe_status_rules -v`  
Expected: FAIL with missing classifier

**Step 3: Write minimal implementation**

实现：
- 探测请求（记录耗时、状态码、错误类型）
- 分类规则：Binance `200/403` 成功、`429` 失败；OpenAI `200/401/403` 成功、`429` 失败；其他 `2xx/3xx` 成功

**Step 4: Run test to verify it passes**

Run: `cargo test probe_status_rules -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/probe tests/probe_status_rules.rs
git commit -m "实现探测与状态码判定规则\n\n- 新增 HTTP 探测器与结果模型\n- 按目标实现可达判定策略\n- 明确 429 为失败"
```

### Task 5: 评分器（可用性优先）

**Files:**
- Create: `src/score/scorer.rs`
- Modify: `src/score/mod.rs`
- Create: `tests/scorer_rank.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn availability_beats_latency() {
    // 节点A：低延迟但失败率高；节点B：稍慢但成功率高
    // 预期 B 排名更高
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test scorer_rank -v`  
Expected: FAIL (scorer missing)

**Step 3: Write minimal implementation**

实现打分：
- `score = avail*wa + p50*wb + p95*wc + jitter*wd - penalty`
- 默认权重：可用性 0.7，性能总计 0.3
- 连续错误触发惩罚窗口

**Step 4: Run test to verify it passes**

Run: `cargo test scorer_rank -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/score tests/scorer_rank.rs
git commit -m "实现可用性优先评分模型\n\n- 引入可用性/延迟/抖动综合评分\n- 增加连续错误惩罚机制\n- 补充节点排序测试"
```

### Task 6: 选择器与防抖切换

**Files:**
- Create: `src/select/decision.rs`
- Modify: `src/select/mod.rs`
- Create: `tests/selector_stability.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn does_not_switch_without_consecutive_wins() {
    // 单轮更优不切换
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test selector_stability -v`  
Expected: FAIL

**Step 3: Write minimal implementation**

实现规则：
- 连续胜出 `min_wins`
- 优势阈值 `min_improvement`
- 冷却时间 `cooldown_sec`
- 当前节点连续硬失败时紧急切换

**Step 4: Run test to verify it passes**

Run: `cargo test selector_stability -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/select tests/selector_stability.rs
git commit -m "实现防抖切换决策器\n\n- 增加连续胜出与优势阈值约束\n- 引入冷却窗口防止频繁切换\n- 支持连续失败紧急切换"
```

### Task 7: SQLite 持久化层

**Files:**
- Create: `src/store/sqlite.rs`
- Modify: `src/store/mod.rs`
- Create: `migrations/0001_init.sql`
- Create: `tests/store_sqlite.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn persist_and_restore_group_state() {
    // 写入当前节点、最近轮次、冷却时间并可恢复
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test store_sqlite -v`  
Expected: FAIL

**Step 3: Write minimal implementation**

实现表：`rounds`、`probes`、`switch_events`、`group_state`。  
提供 API：保存轮次、保存切换事件、读取组状态。

**Step 4: Run test to verify it passes**

Run: `cargo test store_sqlite -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/store migrations tests/store_sqlite.rs Cargo.toml
git commit -m "实现 SQLite 持久化与状态恢复\n\n- 新增轮次与切换事件表结构\n- 支持组状态持久化与重启恢复\n- 添加存储层回归测试"
```

### Task 8: Runner 调度主循环

**Files:**
- Create: `src/runner/loop.rs`
- Modify: `src/runner/mod.rs`
- Modify: `src/main.rs`
- Create: `tests/runner_tick.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn one_tick_runs_probe_score_select_pipeline() {
    // mock controller + store，验证一轮流程会调用完整链路
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test runner_tick -v`  
Expected: FAIL

**Step 3: Write minimal implementation**

实现 `Runner::tick()` 与 `Runner::run_forever()`：
- 周期执行
- 错误不退出
- 每轮记录日志与结果

**Step 4: Run test to verify it passes**

Run: `cargo test runner_tick -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/runner src/main.rs tests/runner_tick.rs
git commit -m "实现守护调度循环\n\n- 打通探测评分切换主链路\n- 增加单轮与常驻执行模型\n- 保证错误场景持续运行"
```

### Task 9: CLI 与运行模式

**Files:**
- Modify: `src/main.rs`
- Create: `src/cli.rs`
- Create: `tests/cli_modes.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn parse_dry_run_and_once_flags() {
    // --dry-run / --once / --config
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test cli_modes -v`  
Expected: FAIL

**Step 3: Write minimal implementation**

支持模式：
- `--once`：跑一轮退出
- `--dry-run`：只评分不切换
- `--config <path>`：指定配置文件

**Step 4: Run test to verify it passes**

Run: `cargo test cli_modes -v`  
Expected: PASS

**Step 5: Commit**

```bash
git add src/main.rs src/cli.rs tests/cli_modes.rs Cargo.toml
git commit -m "增加 CLI 运行模式\n\n- 支持 once 与 dry-run 调试模式\n- 支持自定义配置路径\n- 增加参数解析测试"
```

### Task 10: 文档与 macOS 常驻部署

**Files:**
- Create: `docs/runbook.md`
- Create: `deploy/macos/com.yanxi.route-warden.plist`
- Create: `examples/config.example.yaml`
- Modify: `README.md`

**Step 1: Write the failing test**

```text
无代码测试；改为执行文档验收清单（手动）
```

**Step 2: Run validation to verify it fails before docs exist**

Run: `test -f docs/runbook.md && exit 1 || exit 0`  
Expected: PASS（表示文档尚未存在）

**Step 3: Write minimal implementation**

补充：
- 安装与运行步骤
- launchd 安装/启动/查看日志
- 常见问题排障（429、timeout、节点抖动）

**Step 4: Run validation to verify it passes**

Run: `test -f docs/runbook.md -a -f deploy/macos/com.yanxi.route-warden.plist -a -f examples/config.example.yaml`  
Expected: PASS

**Step 5: Commit**

```bash
git add docs/runbook.md deploy/macos/com.yanxi.route-warden.plist examples/config.example.yaml README.md
git commit -m "补充运行文档与 macOS 常驻部署配置\n\n- 提供 launchd 部署示例\n- 增加配置模板与排障指南\n- 完善项目使用说明"
```

## 执行顺序约束

- Task 1-3 完成后才能开始 Task 4-6
- Task 7 与 Task 8 可并行，但合并前需统一接口
- Task 9 在 Task 8 后执行
- Task 10 最后执行

## 非目标（第一版不做）

- Web 管理面板
- 复杂告警系统（飞书/Slack）
- 多进程分布式探测
- 自动改写完整 Clash 规则文件

