# route-warden

让不同站点自动走各自更稳定的节点，而不是把所有流量都绑在同一个节点上。

`route-warden` 是一个给 Mihomo/Clash 使用的自动切换守护程序。它不是“全局挑一个最快节点”，而是按不同站点和业务组持续探测、独立决策、自动切换。

典型场景是：

- `OpenAI` 在 A 节点更稳，就让 `RW_OPENAI` 自动切到 A
- `GitHub` 在 B 节点更稳，就让 `RW_GITHUB` 自动切到 B
- 其他网站继续走它们各自更合适的线路，而不是跟着一起切

你不需要盯着延迟表手动切节点，也不用赌“一个节点能不能同时兼顾所有站点”。Route Warden 会持续探测目标站点可用性，按组评分，选择更稳定的节点，并通过冷却和连胜阈值减少抖动切换。

## 它解决什么问题

- 一个节点不可能对所有站点都同样稳定
- 手动切节点很打断工作，而且很快又要切回来
- “全局切一次”会让某些站点变好，另一些站点变差
- 你真正需要的是“不同站点，各走各自更稳的节点”

## 当前能力

- 按目标站点/业务组持续探测节点可用性
- 基于成功率、状态码和延迟做评分与自动切换
- 使用防抖策略减少频繁抖动切换
- 将轮次、探测明细、当前节点和切换事件写入 SQLite
- 支持常驻运行，并可同步 Clash Verge 的 `RW_*` 组与规则模板

## 快速开始

```bash
cargo test

# 预览或写入 Clash Verge 的 RW_* 组与规则
cargo run -- sync-rw-profile --dry-run
cargo run -- sync-rw-profile

# 单轮试跑，观察会如何决策
cargo run -- --once --dry-run

# 本机安装 Release 全局使用
cargo install --path .

route-warden --help
```

`sync-rw-profile` 会同时写入 Clash Verge 的 `Profile Enhancement -> Groups` 与 `Profile Enhancement -> Rules` 文件。
`examples/config.example.yaml` 中的 `probe.proxy_url` 可配置探测代理地址（例如改端口）。
默认配置路径：`~/.route-warden/config.yaml`（可通过 `--config` 覆盖）。

## 工作方式

1. 按配置中的目标站点持续发起探测
2. 统计每个节点对不同站点的可用性、状态码和延迟
3. 对 `RW_OPENAI`、`RW_GITHUB`、`RW_GLOBAL` 等分组分别决策
4. 仅在收益足够明显时切换，避免来回抖动

如果你的配置里把 `chatgpt.com`、`api.openai.com` 指到 `RW_OPENAI`，把 `github.com`、`api.github.com` 指到 `RW_GITHUB`，那么 Route Warden 就会分别为这些分组维护更合适的当前节点。

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
# 如果设置了 KeepAlive, 会被重新拉起
launchctl stop com.yanxi.route-warden
launchctl start com.yanxi.route-warden

# 真正暂停
launchctl bootout gui/$(id -u)/com.yanxi.route-warden

# 标准恢复
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.yanxi.route-warden.plist
```

更多部署细节见 [docs/runbook.md](docs/runbook.md)。
节点生效验证见 [docs/verify-clash-verge-node.md](docs/verify-clash-verge-node.md)。
