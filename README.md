# route-warden

用于 Mihomo/Clash 的节点探测与自动切换守护程序（Rust）。

## 当前能力（MVP）

- 配置加载与校验
- 控制器客户端（读节点/切组）
- 目标探测与状态码分类
- 可用性优先评分
- 防抖切换决策
- SQLite 状态与审计持久化（`group_state` / `switch_events` / `rounds` / `probes`）
- 常驻模式每分钟打印分组当前节点与最近连通率
- Runner 主循环与 CLI 参数

## 快速开始

```bash
cargo test

cargo run -- sync-rw-profile --dry-run
cargo run -- sync-rw-profile
cargo run -- --once --dry-run

# 本机安装 Release 全局使用
cargo install --path .

route-warden --help
```

`sync-rw-profile` 会同时写入 Clash Verge 的 `Profile Enhancement -> Groups` 与 `Profile Enhancement -> Rules` 文件。
`examples/config.example.yaml` 中的 `probe.proxy_url` 可配置探测代理地址（例如改端口）。
默认配置路径：`~/.route-warden/config.yaml`（可通过 `--config` 覆盖）。

## macOS 常驻运行（launchd）

以下步骤适用于已执行过 `cargo install --path .`（`route-warden` 已在 PATH）：

```bash
# 1) 准备配置
mkdir -p ~/.route-warden
cp examples/config.example.yaml ~/.route-warden/config.yaml

# 2) 同步 Clash Verge profile 增强（注意 --config 是顶层参数，要放在子命令前）
route-warden --config ~/.route-warden/config.yaml sync-rw-profile

# 3) 安装 LaunchAgent
cp deploy/macos/com.yanxi.route-warden.plist ~/Library/LaunchAgents/

# 4) 根据本机实际路径修改 plist（可用 which route-warden 查看）
# ProgramArguments[0] -> route-warden 绝对路径（例如 /Users/<you>/.cargo/bin/route-warden）
# ProgramArguments[2] -> /Users/<you>/.route-warden/config.yaml

# 5) 重载并启动
launchctl unload ~/Library/LaunchAgents/com.yanxi.route-warden.plist 2>/dev/null || true
launchctl load ~/Library/LaunchAgents/com.yanxi.route-warden.plist
launchctl start com.yanxi.route-warden

# 6) 查看状态与日志
launchctl list | rg route-warden
tail -f /tmp/route-warden.out.log /tmp/route-warden.err.log
```

停止/重启：

```bash
launchctl stop com.yanxi.route-warden
launchctl start com.yanxi.route-warden
```

更多部署细节见 [docs/runbook.md](docs/runbook.md)。
节点生效验证见 [docs/verify-clash-verge-node.md](docs/verify-clash-verge-node.md)。
