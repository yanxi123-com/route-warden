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

更多部署细节见 [docs/runbook.md](docs/runbook.md)。
节点生效验证见 [docs/verify-clash-verge-node.md](docs/verify-clash-verge-node.md)。
