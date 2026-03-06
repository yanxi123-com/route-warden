# 验证 Clash Verge 节点是否生效

本文用于验证：`route-warden` 或手动切换后，流量是否真的按预期走到指定节点。

## 前置条件

- Clash Verge 正在运行。
- Mihomo Controller 通过 Unix Socket 暴露（默认：`/tmp/verge/verge-mihomo.sock`）。
- 本地代理端口可用（示例：`127.0.0.1:7890`）。
- 已完成一次 profile 增强同步并重载配置：

```bash
route-warden --config ~/.route-warden/config.yaml sync-rw-profile
```

在 Clash Verge 执行一次 profile 重载（或重启内核），使 `RW_*` 组与 rules 生效。

## 1. 30 秒快速验证（推荐）

按顺序执行这 3 条命令：

```bash
# 1) 规则是否存在（chatgpt.com -> RW_OPENAI）
rg "DOMAIN,chatgpt.com,RW_OPENAI" \
"$HOME/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/profiles"/*.yaml

# 2) 组当前节点是否已选中
curl --unix-socket /tmp/verge/verge-mihomo.sock \
  http://localhost/proxies/RW_OPENAI | jq -r '.now'

# 3) 业务请求是否可达（强制经 Clash 端口）
env -u http_proxy -u https_proxy -u all_proxy -u no_proxy -u NO_PROXY \
curl -x http://127.0.0.1:7890 -I https://chatgpt.com
```

若 3 步都正常，通常可判定“规则已下发 + 组已生效 + 请求可达”。

## 2. 控制面验证：查看所有 RW 组当前节点

一次查看所有 `RW_` 前缀策略组的当前 `now`：

```bash
curl --unix-socket /tmp/verge/verge-mihomo.sock \
  http://localhost/proxies \
| jq '.proxies
      | with_entries(select(.key | startswith("RW_")) | .value = .value.now)'
```

如果各组 `now` 等于你期望的节点名，说明“配置已下发”。

## 3. 进阶验证（可选）：检查 rule/chains 命中

如果你还想确认具体命中的规则链路，再执行：

```bash
# 触发请求（强制经 Clash 端口）
env -u http_proxy -u https_proxy -u all_proxy \
curl -x http://127.0.0.1:7890 -I https://chatgpt.com

# 查看该请求在 Mihomo 内部命中的规则与链路
curl --unix-socket /tmp/verge/verge-mihomo.sock \
  http://localhost/connections | jq -r '
  .connections[]
  | select((.metadata.host // "") | test("chatgpt"; "i"))
  | [.metadata.host, .rule, (.chains|join(" -> "))]
  | @tsv
'
```

`chains` 中应出现对应策略组与最终节点名。  
同时 `rule` 应命中你配置的域名规则（例如 `DOMAIN,chatgpt.com,RW_OPENAI`）；若未命中，通常会落到 `MATCH,RW_GLOBAL`。
如果这一步查不到记录，通常是连接已结束（`/connections` 只展示当前活跃连接）。

## 4. 出口验证：公网 IP 是否符合该节点

```bash
env -u http_proxy -u https_proxy -u all_proxy \
curl -x http://127.0.0.1:7890 https://api.ip.sb/ip
```

将返回 IP 与预期节点地区/运营商信息对比（可再查 `ipinfo.io/<ip>`）。

## 常见误判

- 只看 `proxies/<group>.now`：只能证明“控制面已切”，不能证明“业务流量命中该组”。
- `curl` 未显式走 `-x 127.0.0.1:7890`：可能绕过 Clash，导致验证结论错误。
- 连接记录抓取太晚：短连接已结束，建议“先请求、立即查 connections”。
