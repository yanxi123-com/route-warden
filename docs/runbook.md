# Route Warden Runbook

## 1. 构建

```bash
cargo build --release
```

产物：`target/release/route-warden`

## 2. 本地试运行

```bash
./target/release/route-warden --config examples/config.example.yaml --once --dry-run
```

## 3. 常驻运行（macOS launchd）

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

## 4. 常见问题

### 4.1 429 过多
- 当前策略把 `429` 视为失败，会导致节点降分。
- 可提高探测间隔并降低单轮请求次数。

### 4.2 timeout / reset 抖动
- 优先确认当前网络（热点通常比 Wi-Fi 更不稳定）。
- 检查 Tailscale 到跳板链路是否长期走中继。

### 4.3 频繁切换
- 提高 `min_wins` 与 `cooldown_sec`。
- 提高 `min_improvement` 阈值，降低抖动切换。
