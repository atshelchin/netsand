# netsand — 进程级网络沙盒管理器

每个进程独立配置，不同的白名单、不同的代理、OS 级强制隔离。

支持平台：**Linux** / **macOS** / **Windows**

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
  │  OS 层强制: 进程只能连 localhost
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
ips = []

[upstream]
proxy = "socks5://127.0.0.1:1080"
domains = ["gamma-api.polymarket.com"]

[sandbox]
user = "botuser"    # Linux/macOS: 以此用户运行，OS 层限制出站
```

## 两层隔离

| 层 | 机制 | 作用 |
|----|------|------|
| **OS 层** | 内核级强制 | 进程只能连 `127.0.0.1:<port>` |
| **代理层** | 域名白名单 + 403 拒绝 | 只放行配置的域名 |

即使恶意代码用 raw TCP 也无法绕过 OS 层限制。

### 平台实现

| 平台 | OS 层机制 | 权限要求 |
|------|----------|----------|
| **Linux** | `iptables -m owner --uid-owner` 按用户限制出站 | sudo |
| **macOS** | `pf` (Packet Filter) 按用户限制出站 | sudo |
| **Windows** | `netsh advfirewall` 按程序路径限制出站 | Administrator |

## 构建

```bash
cargo build --release    # 3.0MB 二进制
```

## 测试

```bash
cargo test    # 27 tests
```

| 测试文件 | 测试数 | 覆盖 |
|----------|--------|------|
| test_policy | 13 | 域名允许/拒绝/大小写/子域名/IP/localhost/上游路由/URL解析 |
| test_proxy | 6 | CONNECT 拦截/放行、HTTP 拦截、空请求、畸形请求、多 profile 隔离 |
| test_profile | 6 | bot/scraper 配置加载、全量加载、默认端口、无效路径 |
| test_state | 2 | 状态序列化/反序列化、端口查找 |

## 项目结构

```
sandbox/
├── crates/
│   ├── netsand-proxy/        # 代理核心库（零第三方网络库）
│   │   ├── src/config.rs     # Policy, UpstreamProxy
│   │   ├── src/proxy.rs      # CONNECT/HTTP 转发 + SOCKS5
│   │   └── src/resolver.rs   # DNS 解析
│   └── netsand-cli/          # CLI 二进制
│       ├── src/main.rs       # clap 命令行
│       ├── src/daemon.rs     # 后台服务管理
│       ├── src/profile.rs    # TOML 配置加载
│       ├── src/process.rs    # 子进程生命周期
│       ├── src/state.rs      # 运行状态持久化
│       └── src/sandbox/
│           ├── linux.rs      # iptables
│           ├── macos.rs      # pf (Packet Filter)
│           └── windows.rs    # netsh advfirewall
└── profiles/                 # 示例配置
    ├── bot.toml
    └── scraper.toml
```
