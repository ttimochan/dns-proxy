# DNS Proxy

一个支持 DoQ、DoH、DoT、DoH3 协议的 DNS 代理服务器，可以从 SNI 中提取前缀信息并重写后转发到上游服务器。

## 功能特性

- ✅ **DoT (DNS over TLS)** - TCP 853 端口
- ✅ **DoH (DNS over HTTPS)** - TCP 443 端口
- ✅ **DoQ (DNS over QUIC)** - UDP 853 端口
- ⚠️ **DoH3 (DNS over HTTP/3)** - UDP 443 端口（需要证书配置）

## 架构

项目采用模块化设计，支持扩展：

```
src/
├── main.rs              # 启动入口
├── app.rs              # 应用管理
├── config.rs           # 配置管理
├── sni.rs              # SNI trait 定义
├── rewrite.rs          # Rewriter 工厂
├── readers/            # 协议服务器实现
│   ├── doh.rs         # DoH 服务器
│   ├── dot.rs         # DoT 服务器
│   ├── doq.rs         # DoQ 服务器
│   └── doh3.rs        # DoH3 服务器
└── rewriters/         # SNI 重写器实现
    └── base.rs        # 基础重写器
```

### 核心模块

- **`sni.rs`** - 定义 `SniRewriter` trait 和 `RewriteResult` 结构
- **`readers/`** - 各协议的服务器实现，每个协议独立文件
- **`rewriters/`** - SNI 重写器实现，支持自定义重写逻辑
- **`config.rs`** - 配置管理，支持从 TOML 文件加载
- **`app.rs`** - 应用生命周期管理

## 配置说明

### 配置文件格式

复制 `config.toml.example` 为 `config.toml` 并根据需要修改：

```toml
[rewrite]
# 基准域名列表，支持多个域名
# 重写器会从匹配这些基准域名的 hostname 中提取前缀
base_domains = ["example.com", "example.org"]
# 目标域名后缀，提取的前缀会与此后缀组合形成目标主机名
target_suffix = ".example.cn"

[servers]
# DNS over TLS (DoT) - TCP 853
[servers.dot]
enabled = true
bind_address = "0.0.0.0"
port = 853

# DNS over HTTPS (DoH) - TCP 443
[servers.doh]
enabled = true
bind_address = "0.0.0.0"
port = 443

# DNS over QUIC (DoQ) - UDP 853
[servers.doq]
enabled = true
bind_address = "0.0.0.0"
port = 853

# DNS over HTTP/3 (DoH3) - UDP 443
[servers.doh3]
enabled = false
bind_address = "0.0.0.0"
port = 443

[upstream]
# 默认上游服务器
default = "8.8.8.8:853"
# 协议特定的上游服务器（可选，回退到 default）
dot = "8.8.8.8:853"
doh = "https://dns.google/dns-query"
doq = "8.8.8.8:853"
doh3 = "https://dns.google/dns-query"

[tls]
# TLS 证书配置
# 默认证书（可选，当没有找到域名特定证书时使用）
[tls.default]
cert_file = "/path/to/default-cert.pem"
key_file = "/path/to/default-key.pem"

# 为每个基准域名配置独立的证书
[tls.certs.example.com]
cert_file = "/path/to/example-com-cert.pem"
key_file = "/path/to/example-com-key.pem"

[tls.certs.example.org]
cert_file = "/path/to/example-org-cert.pem"
key_file = "/path/to/example-org-key.pem"
```

### SNI 重写逻辑

支持多个基准域名的前缀提取和重写：

**示例 1：** 当收到 SNI 为 `www.example.org` 时：

1. 匹配基准域名：`example.org` ✓
2. 提取前缀：`www`
3. 构建目标主机名：`www.example.cn`
4. 将请求转发到 `www.example.cn`

**示例 2：** 当收到 SNI 为 `api.example.com` 时：

1. 匹配基准域名：`example.com` ✓
2. 提取前缀：`api`
3. 构建目标主机名：`api.example.cn`
4. 将请求转发到 `api.example.cn`

**示例 3：** 当收到 SNI 为 `example.org` 时（无前缀）：

- 不匹配，不进行重写

## 使用方法

### 编译

```bash
cargo build --release
```

### 运行

```bash
# 使用默认配置（从 config.toml 加载，如果不存在则使用默认值）
cargo run

# 或直接运行编译后的二进制文件
./target/release/dns-proxy
```

## 扩展性

### 添加新的协议支持

要添加新的协议支持，请参考 `src/readers/README.md`：

1. 在 `readers/` 目录下创建新的协议文件
2. 实现服务器结构体和 `start()` 方法
3. 在 `readers/mod.rs` 中导出
4. 在 `app.rs` 中添加启动逻辑

### 添加新的重写器

要添加自定义的 SNI 重写逻辑，请参考 `src/rewriters/README.md`：

1. 在 `rewriters/` 目录下创建新的重写器文件
2. 实现 `SniRewriter` trait
3. 在 `rewriters/mod.rs` 中导出
4. 在 `rewrite.rs` 中更新工厂函数（可选）

## TLS 证书配置

支持为每个基准域名配置独立的证书文件。这对于多域名场景非常有用，因为一个证书通常只包含一个或几个域名。

### 配置方式

```toml
[tls]
# 默认证书配置（可选）
# 当没有找到域名特定的证书时使用
[tls.default]
cert_file = "/path/to/default-cert.pem"
key_file = "/path/to/default-key.pem"
# ca_file = "/path/to/default-ca.pem"
require_client_cert = false

# 为每个基准域名配置独立的证书
[tls.certs.example.com]
cert_file = "/path/to/example-com-cert.pem"
key_file = "/path/to/example-com-key.pem"
# ca_file = "/path/to/example-com-ca.pem"
require_client_cert = false

[tls.certs.example.org]
cert_file = "/path/to/example-org-cert.pem"
key_file = "/path/to/example-org-key.pem"
# ca_file = "/path/to/example-org-ca.pem"
require_client_cert = false
```

### 证书选择逻辑

1. 当收到 SNI 请求时，首先查找该域名在 `tls.certs` 中的配置
2. 如果找到，使用该域名的证书配置
3. 如果未找到，回退到 `tls.default` 配置（如果存在）
4. 如果都没有配置，将无法处理 TLS 连接

### 配置说明

- **`cert_file`** - 证书文件路径（PEM 格式，必需）
- **`key_file`** - 私钥文件路径（PEM 格式，必需）
- **`ca_file`** - CA 证书文件路径（可选，用于客户端证书验证）
- **`require_client_cert`** - 是否要求客户端证书（默认：false）

## 待完善功能

- [ ] DoT 协议中完整的 SNI 提取和重写
- [ ] DoQ 协议中 QUIC 连接的 SNI 提取和重写
- [ ] DoH3 协议的完整实现（需要证书配置）
- [ ] TLS 证书动态加载和热重载
- [ ] 更完善的错误处理和日志记录
- [ ] 性能监控和统计

## 依赖

- `tokio` - 异步运行时
- `rustls` / `tokio-rustls` - TLS 支持
- `quinn` - QUIC 支持
- `hyper` / `hyper-util` - HTTP 支持
- `serde` / `toml` - 配置解析
- `tracing` / `tracing-subscriber` - 日志记录
- `anyhow` - 错误处理
- `bytes` - 字节处理
- `http-body-util` - HTTP body 工具

## 许可证

MIT
