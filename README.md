# netsand — 进程级网络沙盒管理器

类似 Deno 的 `--allow-net`，但适用于**任意二进制程序**。

每个进程独立白名单、独立代理、OS 级内核强制隔离。

支持平台：**Linux** / **macOS** / **Windows**

## 为什么需要这个工具

运行第三方代码（交易 bot、爬虫、脚本）时，你无法保证代码不会偷偷把敏感数据（API Key、私钥）发到未知服务器。

**netsand 的解决方案：** 在 OS 内核层面锁死进程的网络出口，只允许连接到你指定的代理端口，代理再按白名单决定是否放行。两层防御，任何一层都无法从用户态绕过。

## 工作原理

```
                    ┌─────────────────────────────────────────┐
                    │            netsand daemon                │
                    │         (单进程, tokio 多 task)           │
                    │                                         │
                    │  :8001 → bot profile (4 domains, socks5)│
                    │  :8002 → scraper profile (2 domains)    │
                    └───────┬──────────────┬──────────────────┘
                            │              │
        ┌───────────────────┘              └───────────────────┐
        ▼                                                      ▼
  ┌──────────┐                                          ┌──────────┐
  │ Bot 进程  │                                          │ Scraper  │
  │          │                                          │          │
  │ OS 限制:  │                                          │ OS 限制:  │
  │ 只能连    │                                          │ 只能连    │
  │ 127.0.0.1│                                          │ 127.0.0.1│
  │ :8001    │                                          │ :8002    │
  └──────────┘                                          └──────────┘
```

### 两层隔离

```
Layer 1: OS 内核强制
┌──────────────────────────────────────────────────────────┐
│ Bot 进程尝试连接外网:                                      │
│                                                          │
│   connect("evil.com:443")                                │
│     → iptables/pf/netsh 拦截 → 丢弃 (内核级, 无法绕过)    │
│                                                          │
│   connect("127.0.0.1:8001")                              │
│     → 允许 (唯一出口)                                     │
└──────────────────────────────────────────────────────────┘
                            │
                            ▼
Layer 2: 代理域名白名单
┌──────────────────────────────────────────────────────────┐
│ 代理收到 CONNECT 请求:                                     │
│                                                          │
│   CONNECT gamma-api.polymarket.com:443                   │
│     → 白名单匹配 → 放行 → 直连或走上游代理                  │
│                                                          │
│   CONNECT evil.com:443                                   │
│     → 白名单不匹配 → 返回 403 Forbidden                   │
└──────────────────────────────────────────────────────────┘
```

**为什么需要两层？**

- 光有 Layer 1（OS 防火墙）：进程只能连 localhost，但无法控制连哪个域名
- 光有 Layer 2（代理白名单）：恶意代码可以绕过代理直接用 `TcpStream::connect()`
- **两层结合**：进程被 OS 锁死在代理端口上，代理被白名单锁死在指定域名上

### 上游代理路由（翻墙支持）

```
Bot → netsand daemon
        ├─ stream.binance.com       → 直连 (国内可达)
        ├─ gamma-api.polymarket.com → socks5://127.0.0.1:1080 → 外网
        └─ evil.com                 → 403 Forbidden
```

白名单域名可以按需选择直连或走上游代理（SOCKS5 / HTTP CONNECT）。

### 各平台 OS 层实现

| 平台 | 机制 | 原理 | 隔离粒度 |
|------|------|------|----------|
| **Linux** | `iptables -m owner --uid-owner` | 内核 netfilter 按 UID 匹配出站包 | 按用户 |
| **macOS** | `pf` (Packet Filter) | BSD 包过滤器按用户匹配 | 按用户 |
| **Windows** | `netsh advfirewall` | Windows 防火墙按程序路径匹配 | 按程序 |

三者都在**内核态**执行，用户态代码无法绕过。

## 使用

```bash
# 列出所有 profiles
netsand --profile-dir profiles profiles

# 验证配置
netsand --profile-dir profiles check bot

# 启动 daemon（每个 profile 一个代理端口）
netsand --profile-dir profiles daemon start --foreground

# 在沙盒内运行命令
netsand run bot -- ./my-trading-bot
netsand run scraper -- curl https://api.coingecko.com/api/v3/ping

# 查看状态
netsand status

# 停止 daemon
netsand daemon stop
```

## Profile 配置

```toml
# profiles/bot.toml
[profile]
name = "bot"
description = "Polymarket trading bot"
listen_port = 8001              # 代理监听端口

[allow]
domains = [                     # 域名白名单（子域名自动允许）
    "gamma-api.polymarket.com",
    "stream.binance.com",
]
ips = ["52.84.123.45"]          # IP 白名单

[upstream]                      # 可选：需要翻墙的域名走上游代理
proxy = "socks5://127.0.0.1:1080"
domains = ["gamma-api.polymarket.com"]

[sandbox]
user = "botuser"                # 以此用户运行，OS 层按用户限制出站
```

## 构建

```bash
cargo build --release    # 3.0MB 单二进制
```

## 测试

```bash
cargo test    # 55 tests
```

| 测试文件 | 测试数 | 覆盖范围 |
|----------|--------|----------|
| test_policy | 20 | 域名允许/拒绝/大小写/子域名/偏匹配/空策略/IPv4/IPv6/上游路由/上游IP/URL解析/host:port解析 |
| test_proxy | 17 | CONNECT 拦截/放行/IP拦截/IP放行/自定义端口/HTTP拦截/POST拦截/路径查询/空请求/畸形请求/部分请求/二进制垃圾/多profile隔离/run_listener/性能基准(3) |
| test_profile | 9 | bot/scraper加载/全量加载/默认端口/无效路径/无效目录/畸形TOML/缺必填字段/最小配置 |
| test_state | 5 | 端口查找/空状态/序列化往返/保存加载/默认值 |
| test_resolver | 4 | localhost解析/IP解析/无效域名/端口保留 |

## 项目结构

```
sandbox/
├── crates/
│   ├── netsand-proxy/            # 代理核心库（零第三方网络库）
│   │   ├── src/config.rs         # Policy, UpstreamProxy, 解析
│   │   ├── src/proxy.rs          # CONNECT/HTTP 转发 + SOCKS5 握手
│   │   ├── src/resolver.rs       # DNS 解析
│   │   └── tests/                # 41 tests
│   └── netsand-cli/              # CLI 二进制
│       ├── src/main.rs           # clap 命令行入口
│       ├── src/daemon.rs         # 后台服务 start/stop
│       ├── src/profile.rs        # TOML 配置加载
│       ├── src/process.rs        # 沙盒子进程生命周期
│       ├── src/state.rs          # 运行状态持久化
│       ├── src/sandbox/
│       │   ├── linux.rs          # iptables 实现
│       │   ├── macos.rs          # pf 实现
│       │   └── windows.rs        # netsh 实现
│       └── tests/                # 14 tests
└── profiles/                     # 示例配置
    ├── bot.toml
    └── scraper.toml
```
