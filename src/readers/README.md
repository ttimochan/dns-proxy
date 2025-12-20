# Readers 模块

Readers 模块包含各个协议的服务器实现。每个 reader 负责：

- 监听指定端口
- 接收客户端请求
- 提取 SNI 信息
- 使用 rewriter 重写 SNI
- 转发请求到上游服务器

## 添加新的 Reader

要添加新的协议支持，请按以下步骤：

1. 在 `readers/` 目录下创建新的文件，例如 `my_protocol.rs`

2. 实现服务器结构体：

```rust
use crate::config::AppConfig;
use crate::rewrite::SniRewriterType;
use anyhow::Result;
use tracing::info;

pub struct MyProtocolServer {
    config: AppConfig,
    rewriter: SniRewriterType,
}

impl MyProtocolServer {
    pub fn new(config: AppConfig, rewriter: SniRewriterType) -> Self {
        Self { config, rewriter }
    }

    pub async fn start(&self) -> Result<()> {
        // 实现服务器启动逻辑
        Ok(())
    }
}
```

3. 在 `readers/mod.rs` 中添加模块和导出：

```rust
pub mod my_protocol;
pub use my_protocol::MyProtocolServer;
```

4. 在 `app.rs` 中添加启动逻辑

## 现有 Readers

- `doh.rs` - DNS over HTTPS (DoH) 服务器
- `dot.rs` - DNS over TLS (DoT) 服务器
- `doq.rs` - DNS over QUIC (DoQ) 服务器
- `doh3.rs` - DNS over HTTP/3 (DoH3) 服务器
