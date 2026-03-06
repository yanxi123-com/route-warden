# Route Warden 设计文档

日期：2026-03-06  
项目：`route-warden`

## 1. 目标与范围

### 1.1 目标
构建一个在 macOS 常驻运行的 Rust 程序，基于 Clash/Mihomo External Controller 定期探测多站点可用性与性能，按分组自动选择最优节点并切换，优先保证稳定性；后续可无缝扩展到 Ubuntu。

### 1.2 第一版目标站点
- Google 网站：`google.com`
- Binance API：`api.binance.com`
- ChatGPT 网站：`chatgpt.com`
- OpenAI API：`api.openai.com`
- GitHub 网站与 API：`github.com`、`api.github.com`

### 1.3 路由策略目标
- 国内网站：`DIRECT`
- 特定网站：走各自独立策略组的最优节点
- 其他流量：走全局最优策略组

## 2. 总体架构

程序采用单进程常驻守护架构（推荐方案）。

模块划分：
- `Config`：加载与校验配置
- `Probe`：执行组内节点探测
- `Scorer`：基于可用性与性能评分
- `Selector`：执行防抖与切换判定
- `Switcher`：调用 Mihomo API 切换策略组
- `Store`：SQLite 持久化
- `Runner`：调度循环与健康控制

第一版边界（YAGNI）：
- 节点候选范围默认“全部可用节点”
- 不做 UI，仅 CLI + 日志 + SQLite
- 不改写完整 Clash 配置，仅切换目标策略组当前节点

## 3. 数据流与核心算法

### 3.1 轮询流程
每轮（如每 3 分钟）执行：
1. 拉取 Mihomo 节点与策略组状态
2. 按组并发探测候选节点
3. 计算“组-节点”分数
4. 基于稳定策略判定是否切换
5. 落库轮次结果与事件

### 3.2 分组与目标映射
- `GOOGLE_GROUP` -> `google.com`
- `BINANCE_GROUP` -> `api.binance.com`
- `OPENAI_GROUP` -> `chatgpt.com` + `api.openai.com`
- `GITHUB_GROUP` -> `github.com` + `api.github.com`
- `GLOBAL_BEST` -> 全量目标集合

### 3.3 可用性判定（优先级高于延迟）
按目标定义状态码规则：
- Google/ChatGPT/GitHub 网站：`2xx/3xx` 成功
- Binance API：`200`、`403` 成功；`429` 失败
- OpenAI API：`200`、`401`、`403` 成功；`429` 失败
- 其余 `5xx/timeout/reset/tls fail` 失败

### 3.4 评分模型
综合分由以下组成（可配置）：
- 可用性分（最高权重）
- 延迟分（P50）
- 尾延迟分（P95）
- 抖动惩罚
- 错误惩罚（连续 `5xx/timeout/reset` 提升惩罚）

建议初始权重：可用性 70%，性能 30%。

### 3.5 稳定切换策略
- 新节点需“连续 N 轮胜出”
- 分数优势需超过阈值（如 15%）
- 需满足冷却窗口（如 10 分钟）
- 当前节点连续硬失败达到阈值时，允许紧急切换到次优节点

## 4. 配置模型

配置文件：`config.yaml`

核心字段：
- `controller`：Mihomo 地址与鉴权
- `interval_sec`、`cooldown_sec`、`min_wins`、`min_improvement`
- `groups`：5 个逻辑组定义
- `targets`：URL、方法、超时、状态码规则
- `scoring`：权重与惩罚参数
- `routing`：域名到组映射
- `logging`：级别、文件、保留

后续可扩展：为每组添加节点候选白名单（第一版不启用）。

## 5. 错误处理与运行稳定性

- Mihomo API 不可达：本轮不切换，记录错误，下轮重试
- 错误分类：`timeout` / `tcp reset` / `tls fail` / `http status`
- `429` 统一按失败处理
- 单目标失败不立即淘汰节点，按组综合分决策
- 切换事件写审计日志（前后节点、分差、触发原因）
- SQLite 持久化确保重启后可恢复冷却状态与历史上下文

平台托管：
- macOS：`launchd`
- Ubuntu：后续 `systemd`

## 6. 测试与验收标准

### 6.1 功能验收
- 能周期性探测并入库
- 能按组评分并独立切换
- 仅在满足防抖条件时切换
- Binance `403` 可达，`429` 失败

### 6.2 稳定性验收
- 连续运行 24 小时无崩溃
- Mihomo 短暂故障时不退出，恢复后继续
- 节点大面积异常时不抖动切换

### 6.3 正确性验收
- 通过模拟慢节点/错误码验证评分与切换结果
- 每次切换原因可追溯

### 6.4 性能验收
- 单轮探测耗时可控（目标 < 30 秒）
- 常驻 CPU/内存维持低负载

## 7. 里程碑建议

- M1：最小可运行守护（探测 + 评分 + 切组）
- M2：SQLite 与审计日志
- M3：launchd 部署与稳定性调优
- M4：Ubuntu systemd 适配
