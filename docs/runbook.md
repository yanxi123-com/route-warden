# Route Warden Runbook

## 1. 构建

```bash
cargo build --release
```

产物：`target/release/route-warden`
默认配置路径：`~/.route-warden/config.yaml`（可通过 `--config` 覆盖）。

## 2. 同步 Clash Verge 的 RW 组模板

先把 `RW_*` 组写入 `Profile Enhancement -> Groups`（避免手工维护）。

```bash
# 只看将改哪些文件（默认当前 profile）
./target/release/route-warden sync-rw-groups --dry-run

# 写入当前 profile
./target/release/route-warden sync-rw-groups

# 写入所有远程订阅绑定的 groups 文件
./target/release/route-warden sync-rw-groups --all
```

默认 Clash Verge 目录：`~/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev`  
可通过 `--verge-dir` 指定其他目录。

执行后在 Clash Verge 中重载当前 profile（或重启内核），使组配置生效。

## 3. 本地试运行

```bash
./target/release/route-warden --once --dry-run
```

`examples/config.example.yaml` 关键项：
- `controller.base_url`：Clash controller 地址（支持 `unix:///tmp/verge/verge-mihomo.sock`）
- `probe.proxy_url`：探测流量走的本地代理地址（默认 `http://127.0.0.1:7890`，可改端口）

每轮会写入 SQLite（与配置同目录同名 `.sqlite3`）：
- `rounds`：轮次开始/结束与状态
- `probes`：每次探测明细
- `group_state`：各组当前节点与冷却状态
- `switch_events`：切换事件日志

## 4. 常驻运行（macOS launchd）

1. 修改 `deploy/macos/com.yanxi.route-warden.plist` 中的路径：
- `ProgramArguments` 指向本机二进制与配置
- `StandardOutPath`、`StandardErrorPath` 指向可写日志路径

2. 加载服务：

```bash
launchctl unload ~/Library/LaunchAgents/com.yanxi.route-warden.plist 2>/dev/null || true
cp deploy/macos/com.yanxi.route-warden.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.yanxi.route-warden.plist
launchctl start com.yanxi.route-warden
```

3. 查看状态与日志：

```bash
launchctl list | rg route-warden
log show --last 10m --predicate 'process == "route-warden"'
```

## 5. 常见问题

### 5.1 429 过多
- 当前策略把 `429` 视为失败，会导致节点降分。
- 可提高探测间隔并降低单轮请求次数。

### 5.2 timeout / reset 抖动
- 优先确认当前网络（热点通常比 Wi-Fi 更不稳定）。
- 检查 Tailscale 到跳板链路是否长期走中继。

### 5.3 频繁切换
- 提高 `min_wins` 与 `cooldown_sec`。
- 提高 `min_improvement` 阈值，降低抖动切换。

### 5.4 sync-rw-groups 执行后看不到 RW 组
- 确认命令输出的目标文件是当前 profile 绑定的 groups 文件。
- 在 Clash Verge 执行一次 profile 重载或重启内核。
- 使用 controller 检查：

```bash
curl --unix-socket /tmp/verge/verge-mihomo.sock http://localhost/proxies \
| jq -r '.proxies | keys[] | select(startswith("RW_"))'
```
