# Single Probe Multi-Group Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将每轮探测从“每组重复探测”改为“全局仅探测一次并复用到各业务组决策”，在不改变组级切换语义的前提下降低轮次耗时。  
**Architecture:** 新增共享探测阶段，基于所有组的节点并集与目标并集执行一次 `RW_PROBE` 节点遍历；随后按组投影共享结果生成 `NodeStats` 并沿用原有 `apply_group_round` 决策与切换逻辑。  
**Tech Stack:** Rust 2024, tokio, reqwest, anyhow, existing `ControllerClient`/`score`/`select`/`store` modules.

---

### Task 1: 为共享探测引入失败测试

**Files:**
- Modify: `src/app.rs`

**Step 1: Write the failing test**

新增测试覆盖：
- 单轮探测时，每个节点只切一次 `RW_PROBE`（不因组数重复切换）
- 同一轮下，按组决策仍基于各自 targets 生成独立 `stats`

**Step 2: Run test to verify it fails**

Run: `cargo test probe_ -- --nocapture`  
Expected: FAIL（现有实现按组重复探测，不满足断言）

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "test: add failing tests for shared probe round reuse"
```

### Task 2: 实现共享探测并改造组决策输入

**Files:**
- Modify: `src/app.rs`

**Step 1: Write minimal implementation**

实现要点：
- 新增全局探测函数：构建共享节点/目标并集并执行一次 `RW_PROBE` 探测
- 生成共享样本缓存（`(node,target)->sample`）
- 按组从共享缓存投影为 `GroupProbeRound`（含组内 `stats` 与 `probes`）
- 保持 `apply_group_round` 切换逻辑不变（`keep/switched/switch_events/group_state` 语义不变）

**Step 2: Run test to verify it passes**

Run: `cargo test probe_ -- --nocapture`  
Expected: PASS

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "refactor: probe once per round and reuse results across groups"
```

### Task 3: 回归验证与文档对齐

**Files:**
- Modify: `docs/plans/2026-03-10-single-probe-multi-group-design.md`（如实现细节与设计有偏差时）

**Step 1: Run verification**

Run:
- `cargo fmt --check`
- `cargo test detect_external_node_drift -- --nocapture`
- `cargo test app::tests::minute_report_uses_group_state_and_probe_summary -- --nocapture`
- `cargo test app::tests::probe_uses_probe_group_instead_of_strategy_group -- --nocapture`

Expected: 全部 PASS

**Step 2: Update docs if needed**

如实现细节变化，更新设计文档的数据结构与流程描述。

**Step 3: Commit**

```bash
git add docs/plans src/app.rs
git commit -m "docs: align single-probe design with implementation details"
```
