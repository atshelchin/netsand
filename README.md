# netsand — 进程级网络沙盒管理器

每个进程独立配置，不同的白名单、不同的代理、OS 级隔离。

## 使用

```bash
# 列出 profiles
netsand --profile-dir profiles profiles

# 验证配置
netsand --profile-dir profiles check bot

# 启动 daemon（加载所有 profiles，每个 profile 一个代理端口）
netsand --profile-dir profiles daemon start --foreground

# 沙盒内运行命令
netsand run bot -- ./my-trading-bot
netsand run scraper -- curl https://api.coingecko.com/api/v3/ping

# 查看状态
netsand status

# 停止 daemon
netsand daemon stop
```

## 架构

```
netsand daemon (单进程, tokio 多 task)
  ├─ :8001 → bot profile       4 domains, via socks5
  ├─ :8002 → scraper profile   2 domains, direct
  └─ :800N → ...

netsand run bot -- ./bot
  │  设置 HTTP_PROXY=127.0.0.1:8001
  │  Linux: iptables --uid-owner botuser → localhost only
  │  macOS: sandbox-exec → localhost only
  └─ ./bot (只能通过代理访问白名单域名)
```

## Profile 配置

```toml
# profiles/bot.toml
[profile]
name = "bot"
description = "Polymarket trading bot"
listen_port = 8001

[allow]
domains = [
    "gamma-api.polymarket.com",
    "stream.binance.com",
]

[upstream]
proxy = "socks5://127.0.0.1:1080"
domains = ["gamma-api.polymarket.com"]

[sandbox]
user = "botuser"    # Linux: 以此用户运行，iptables 限制出站
```

## 两层隔离

| 层 | 机制 | 作用 |
|----|------|------|
| **OS 层** | iptables (Linux) / sandbox-exec (macOS) | 进程只能连 `127.0.0.1:<port>` |
| **代理层** | 域名白名单 + 403 拒绝 | 只放行配置的域名 |

即使恶意代码用 raw TCP 也无法绕过 OS 层限制。

## 构建

```bash
cargo build --release    # 3.0MB 二进制
```
