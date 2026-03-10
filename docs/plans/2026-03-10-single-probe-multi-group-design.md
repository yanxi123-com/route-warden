# Route Warden 单次探测多组复用设计

日期：2026-03-10  
项目：`route-warden`

## 1. 目标

在不改变“按业务组独立决策/独立切换”语义的前提下，将探测阶段从“每组重复探测一次”改为“每轮只探测一次，结果给各组复用”，缩短决策间隔并降低 `RW_PROBE` 频繁切换开销。

## 2. 当前逻辑与问题

当前每轮流程（简化）：
1. 遍历 `groups`（`OPENAI_GROUP`/`GITHUB_GROUP`/...）
2. 对每个组读取该组 `strategy_group` 的成员节点
3. 使用 `RW_PROBE` 逐节点切换并探测该组 targets
4. 仅基于该组探测结果评分与决策
5. 切换该组 `strategy_group`（如需要）

问题：
- 多个组会对同一批节点重复执行探测与 `RW_PROBE` 切换。
- 轮次总时长随 `组数 x 节点数` 增长，导致有效决策间隔显著大于 `interval_sec`。
- `min_wins` 的真实时间窗口被拉长（例如 3 连胜可能变成 40+ 分钟）。

## 3. 目标方案（Probe Once, Decide Many）

### 3.1 总体流程

每轮改为两个阶段：

1. 探测阶段（全局一次）  
   - 聚合所有业务组需要的节点全集与 target 全集。  
   - 仅用 `RW_PROBE` 对节点全集逐节点切换并探测 target 全集。  
   - 产出全局探测结果缓存（本轮内存结构 + 明细落库）。

2. 决策阶段（按组独立）  
   - 对每个业务组，从全局缓存中筛选“该组节点集合 + 该组 targets”的子集。  
   - 计算该组 `NodeStats -> score -> decision`。  
   - 若触发 `Switch`，切换该组 `strategy_group`；否则 `Keep`。  
   - 继续保持每组独立 `min_wins / cooldown / switch_events / group_state`。

### 3.2 关键点

- `RW_PROBE` 仅承担探测出口，不承担业务切换。
- 业务组切换目标仍是 `RW_OPENAI`、`RW_GITHUB` 等策略组。
- 同一轮内不再重复探测；下一轮仍正常重探。

## 4. 数据结构建议

新增本轮共享探测结构（示意）：

```rust
struct SharedProbeRound {
    probe_group: String,              // RW_PROBE
    probe_group_original_node: String,
    observed_at: i64,
    // key: (node, target_name)
    results: HashMap<(String, String), ProbeSample>,
}
```

其中 `ProbeSample` 复用现有字段：
- `status_code`
- `latency_ms`
- `is_success`
- `failure_kind`
- `created_at`

决策阶段为每组构造：
- `group_nodes`: 该组 `strategy_group` 成员
- `group_targets`: 该组 targets
- 从 `SharedProbeRound.results` 投影为该组 `NodeStats`

## 5. 复杂度变化（以当前配置估算）

已知：
- 业务组数量 `G = 5`
- 每组节点数约 `N = 143`
- 各组 target 数：`1,1,2,2,2`，总和 8，去重后约 6

当前（按组重复探测）：
- 目标请求量约：`N * 8 = 1144 / 轮`
- `RW_PROBE` 切换 PUT 约：`G * (N + 1) = 720 / 轮`

方案后（单次探测复用）：
- 目标请求量约：`N * 6 = 858 / 轮`
- `RW_PROBE` 切换 PUT 约：`N + 1 = 144 / 轮`

收益：
- 探测组切换请求减少约 80%
- 探测请求减少约 25%
- 轮次时长显著下降，`min_wins` 对应真实时间窗口缩短

## 6. 日志与可观测性

保留现有：
- `keep: ... reason=...`
- `switched: ...`
- `external-node-drift: ...`

建议新增：
- `probe-round-summary: nodes=..., targets=..., requests=..., duration_ms=...`
- `group-decision-input: group=..., nodes=..., targets=...`

用于验证“探测一次、决策多次”是否按预期执行。

## 7. 边界与失败处理

- 若 `RW_PROBE` 切换失败：本轮标记失败，记录 `round failed`，不执行后续组决策。
- 若单个 target 探测失败：记录失败样本，不中断整轮。
- 若某组投影后无可评分节点：该组跳过并告警，不影响其他组。
- 探测结束必须恢复 `RW_PROBE` 原节点；恢复失败单独报错。

## 8. 与调度语义的关系

本设计仅解决“单轮执行太重”的核心问题。  
可叠加调度优化（下一步）：
- 将 `interval_sec` 解释为“轮次开始最小间隔”
- 若 `round_duration >= interval_sec`，下一轮立即开始，不再额外 sleep

## 9. 验收标准

1. 同一轮日志中仅出现一次完整 `RW_PROBE` 节点遍历。  
2. 各业务组仍有独立 `keep/switched` 决策日志。  
3. `switch_events` 语义不变（仅记录业务组实际切换）。  
4. 在相同配置下，最近 50 轮平均间隔显著下降。  
5. 节点选择结果与旧逻辑在同等输入下保持一致（除时间顺序差异）。
