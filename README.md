# route-warden

用于 Mihomo/Clash 的节点探测与自动切换守护程序（Rust）。

## 当前能力（MVP）

- 配置加载与校验
- 控制器客户端（读节点/切组）
- 目标探测与状态码分类
- 可用性优先评分
- 防抖切换决策
- SQLite 状态持久化
- Runner 主循环与 CLI 参数

## 快速开始

```bash
cargo test
cargo run -- sync-rw-groups --dry-run
cargo run -- sync-rw-groups
cargo run -- --config examples/config.example.yaml --once --dry-run
```

`sync-rw-groups` 用于把 `RW_*` 组模板写入 Clash Verge 的 `Profile Enhancement -> Groups` 文件。

更多部署细节见 [docs/runbook.md](docs/runbook.md)。
