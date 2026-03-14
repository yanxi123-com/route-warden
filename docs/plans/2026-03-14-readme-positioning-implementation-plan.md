# README Positioning Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `README.md` 首页改写为“产品介绍优先、使用说明紧随其后”的结构，突出“不同站点自动切换到更稳定节点”的核心价值。  
**Architecture:** 保留现有命令、部署和文档链接，但重排 README 结构；先写一句价值主张和典型场景，再补能力摘要与操作说明，避免继续以 MVP 技术清单开场。  
**Tech Stack:** Markdown, existing repository docs (`README.md`, `docs/runbook.md`, `docs/verify-clash-verge-node.md`).

---

### Task 1: 落 README 定位文档

**Files:**
- Create: `docs/plans/2026-03-14-readme-positioning-design.md`

**Step 1: Write the design summary**

写清楚：
- 核心卖点是“不同站点自动走各自更稳定的节点”
- 必须明确举出 `OpenAI`、`GitHub`、其他站点的例子
- README 结构要从“功能清单”改为“价值主张 + 场景 + 使用说明”

**Step 2: Review the file**

Run: `sed -n '1,240p' docs/plans/2026-03-14-readme-positioning-design.md`  
Expected: 设计目标、结构、边界和验收标准完整可读

**Step 3: Commit**

```bash
git add docs/plans/2026-03-14-readme-positioning-design.md
git commit -m "docs: add README positioning design"
```

### Task 2: 重写 README 首页结构

**Files:**
- Modify: `README.md`

**Step 1: Rewrite the top sections**

重写 README 前半部分，包含：
- 一句话价值主张
- 典型场景说明（`OpenAI` / `GitHub` / 其他站点）
- “解决什么问题”或等价小节
- 用户收益导向的能力摘要

**Step 2: Keep operational instructions intact**

保留并整理：
- `cargo test`
- `cargo run -- sync-rw-profile`
- `cargo run -- --once --dry-run`
- `cargo install --path .`
- `launchd` 常驻运行步骤
- 相关文档链接

**Step 3: Review the diff**

Run: `git diff -- README.md`  
Expected: README 首页定位明显转向产品介绍，且使用说明没有丢失

**Step 4: Commit**

```bash
git add README.md
git commit -m "docs: reposition README around per-site stable routing"
```

### Task 3: 最终校验

**Files:**
- Modify: `README.md`（如需微调）

**Step 1: Verify formatting and content**

Run:
- `sed -n '1,260p' README.md`
- `git diff --check`

Expected:
- README 顶部文案清晰
- 无 patch 格式错误或尾随空格

**Step 2: Verify against design**

逐项核对：
- 是否明确写出不同站点自动切不同节点
- 是否包含 `OpenAI` / `GitHub` 示例
- 是否仍保留快速开始和常驻说明

**Step 3: Commit if adjusted**

```bash
git add README.md docs/plans/2026-03-14-readme-positioning-implementation-plan.md
git commit -m "docs: finalize README positioning plan and copy"
```
