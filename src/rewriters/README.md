# Rewriters 模块

Rewriters 模块包含 SNI 重写器的实现。每个 rewriter 负责：

- 从 SNI 中提取前缀
- 根据配置重写 SNI
- 构建目标主机名

## 添加新的 Rewriter

要添加新的重写逻辑，请按以下步骤：

1. 在 `rewriters/` 目录下创建新的文件，例如 `custom.rs`

2. 实现 `SniRewriter` trait：

```rust
use crate::sni::{RewriteResult, SniRewriter};
use crate::config::RewriteConfig;
use anyhow::Result;

pub struct CustomSniRewriter {
    config: RewriteConfig,
    // 其他字段
}

impl CustomSniRewriter {
    pub fn new(config: RewriteConfig) -> Self {
        Self { config }
    }
}

impl SniRewriter for CustomSniRewriter {
    async fn rewrite(&self, sni: &str) -> Option<RewriteResult> {
        // 实现自定义重写逻辑
        None
    }
}
```

3. 在 `rewriters/mod.rs` 中添加模块和导出：

```rust
pub mod custom;
pub use custom::CustomSniRewriter;
```

4. 在 `rewrite.rs` 中更新工厂函数（可选）

## 现有 Rewriters

- `base.rs` - 基础 SNI 重写器，支持多基准域名前缀提取和重写
