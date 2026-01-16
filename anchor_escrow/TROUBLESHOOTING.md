# Anchor Escrow 项目踩坑记录

本文档记录了在实现 Blueshift Anchor Escrow 挑战过程中遇到的所有问题及解决方案。

---

## 目录

1. [构建工具链问题](#1-构建工具链问题)
2. [栈溢出问题](#2-栈溢出问题)
3. [程序 ID 不匹配](#3-程序-id-不匹配)
4. [指令判别符不匹配](#4-指令判别符不匹配)
5. [Token Interface 兼容性问题](#5-token-interface-兼容性问题)
6. [账户顺序问题（关键）](#6-账户顺序问题关键)
7. [账户初始化问题](#7-账户初始化问题)

---

## 1. 构建工具链问题

### 错误信息
```
error: the 'edition2024' feature has been stabilized since 1.85.0
```

### 原因
`constant_time_eq v0.4.2` crate 需要 Rust edition 2024 特性，但默认的 `anchor build` 使用的工具链版本过旧。

### 解决方案
使用 `cargo-build-sbf` 并指定较新的工具版本：

```bash
cargo-build-sbf --tools-version 1.52
```

### 注意事项
- 不要使用 `anchor build`，它可能使用旧版本工具链
- `--tools-version 1.52` 包含了对 edition 2024 的支持

---

## 2. 栈溢出问题

### 错误信息
```
Stack offset of 4208 exceeded max offset of 4096 by 112 bytes
```

### 原因
Solana 程序栈大小限制为 4KB。当一个指令有多个账户时，每个 `InterfaceAccount` 或 `Account` 类型会占用栈空间。

### 解决方案
使用 `Box<>` 将大型账户移到堆上：

```rust
// 之前（栈分配）
pub mint_a: InterfaceAccount<'info, Mint>,
pub mint_b: InterfaceAccount<'info, Mint>,

// 之后（堆分配）
pub mint_a: Box<Account<'info, Mint>>,
pub mint_b: Box<Account<'info, Mint>>,
```

### 适用场景
- 当指令有 5 个以上账户时
- 特别是 `Take` 指令，通常有 10+ 个账户

---

## 3. 程序 ID 不匹配

### 错误信息
```
ERROR: Custom program error: 0x1004
DeclaredProgramIdMismatch
```

### 原因
`declare_id!` 宏中的程序 ID 与测试环境期望的不一致。

### 解决方案
根据测试环境要求设置正确的程序 ID：

```rust
// Blueshift 测试环境使用的占位符 ID
declare_id!("22222222222222222222222222222222222222222222");
```

### 注意事项
- 不同测试环境可能需要不同的程序 ID
- 生产环境需要使用真实生成的密钥对

---

## 4. 指令判别符不匹配

### 错误信息
```
ERROR: Custom program error: 0x65
InstructionFallbackNotFound
```

### 原因
Anchor 默认使用 `sha256("global:<函数名>")[0..8]` 作为判别符，但 Blueshift 期望使用自定义数字判别符。

### 解决方案
在 Anchor 0.31+ 中使用 `#[instruction(discriminator = N)]` 属性：

```rust
#[program]
pub mod anchor_escrow {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn make(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> {
        // ...
    }

    #[instruction(discriminator = 1)]
    pub fn take(ctx: Context<Take>) -> Result<()> {
        // ...
    }

    #[instruction(discriminator = 2)]
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        // ...
    }
}
```

对于账户状态也需要自定义判别符：

```rust
#[account(discriminator = 1)]
#[derive(InitSpace)]
pub struct Escrow {
    // ...
}
```

### 技术细节
- 数字判别符会被转换为 8 字节小端格式
- `discriminator = 1` 实际上是 `[1, 0, 0, 0, 0, 0, 0, 0]`

---

## 5. Token Interface 兼容性问题

### 错误信息
```
ERROR: Custom program error: 0xbc4
AccountNotInitialized (Error Number: 3012)
```

### 原因
`anchor_spl::token_interface` 模块用于 Token-2022 兼容，验证更严格。如果测试环境使用标准 SPL Token，可能导致兼容性问题。

### 解决方案
使用标准 `anchor_spl::token` 模块：

```rust
// 之前（Token-2022 兼容）
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, ...};
pub mint_a: InterfaceAccount<'info, Mint>,
pub token_program: Interface<'info, TokenInterface>,

// 之后（标准 Token）
use anchor_spl::token::{Mint, Token, TokenAccount, ...};
pub mint_a: Account<'info, Mint>,
pub token_program: Program<'info, Token>,
```

### 同时移除的约束
```rust
// 移除这些约束
#[account(mint::token_program = token_program)]
associated_token::token_program = token_program,
```

---

## 6. 账户顺序问题（关键）

### 错误信息
```
ERROR: Custom program error: 0xbbf
AccountOwnedByWrongProgram (Error Number: 3007)
Left: 22222222222222222222222222222222222222222222  // 我们的程序
Right: TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA  // Token 程序
```

### 原因
**这是最关键的问题！** Anchor 按照 struct 中定义的顺序传递账户。如果顺序与客户端期望的不一致，会导致账户类型验证失败。

例如，如果客户端按 `[maker, escrow, mint_a, ...]` 顺序传递账户，但我们的 struct 定义为 `[maker, mint_a, escrow, ...]`，那么 `escrow` 账户会被当作 `mint_a` 处理，导致所有者验证失败。

### 解决方案
严格按照文档定义账户顺序：

**Make 指令账户顺序：**
```rust
pub struct Make<'info> {
    pub maker: Signer<'info>,           // 1
    pub escrow: Account<'info, Escrow>, // 2
    pub mint_a: Account<'info, Mint>,   // 3
    pub mint_b: Account<'info, Mint>,   // 4
    pub maker_ata_a: Account<'info, TokenAccount>, // 5
    pub vault: Account<'info, TokenAccount>,       // 6
    pub associated_token_program: Program<'info, AssociatedToken>, // 7
    pub token_program: Program<'info, Token>,      // 8
    pub system_program: Program<'info, System>,    // 9
}
```

**Take 指令账户顺序：**
```rust
pub struct Take<'info> {
    pub taker: Signer<'info>,            // 1
    pub maker: SystemAccount<'info>,     // 2
    pub escrow: Box<Account<'info, Escrow>>, // 3
    pub mint_a: Box<Account<'info, Mint>>,   // 4
    pub mint_b: Box<Account<'info, Mint>>,   // 5
    pub vault: Box<Account<'info, TokenAccount>>,     // 6
    pub taker_ata_a: Box<Account<'info, TokenAccount>>, // 7
    pub taker_ata_b: Box<Account<'info, TokenAccount>>, // 8
    pub maker_ata_b: Box<Account<'info, TokenAccount>>, // 9
    pub associated_token_program: Program<'info, AssociatedToken>, // 10
    pub token_program: Program<'info, Token>,  // 11
    pub system_program: Program<'info, System>, // 12
}
```

**Refund 指令账户顺序：**
```rust
pub struct Refund<'info> {
    pub maker: Signer<'info>,           // 1
    pub escrow: Account<'info, Escrow>, // 2
    pub mint_a: Account<'info, Mint>,   // 3
    pub vault: Account<'info, TokenAccount>,    // 4
    pub maker_ata_a: Account<'info, TokenAccount>, // 5
    pub associated_token_program: Program<'info, AssociatedToken>, // 6
    pub token_program: Program<'info, Token>,   // 7
    pub system_program: Program<'info, System>, // 8
}
```

### 调试技巧
- 当看到 `AccountOwnedByWrongProgram` 错误时，检查错误信息中的 "Left" 和 "Right"
- 如果 "Left" 是你的程序 ID，说明传入的是你程序创建的账户（如 PDA）
- 这通常意味着账户顺序错位

---

## 7. 账户初始化问题

### 错误信息
```
ERROR: Custom program error: 0xbc4
AccountNotInitialized for maker_ata_a
```

### 原因
在 Refund 指令中，`maker_ata_a` 可能在调用时尚未创建。虽然 Make 指令从 `maker_ata_a` 转出代币，但某些测试场景可能在 Refund 时 ATA 不存在。

### 解决方案
使用 `init_if_needed` 属性：

```rust
#[account(
    init_if_needed,
    payer = maker,
    associated_token::mint = mint_a,
    associated_token::authority = maker,
)]
pub maker_ata_a: Account<'info, TokenAccount>,
```

### 注意事项
- 需要在 `Cargo.toml` 中启用 `init-if-needed` feature：
  ```toml
  anchor-lang = { version = "0.32.1", features = ["init-if-needed"] }
  ```
- 使用 `init_if_needed` 时需要指定 `payer`

---

## 总结清单

在开发 Anchor 程序时，按以下顺序检查：

1. **构建工具链** - 确保使用兼容的 Solana 工具版本
2. **栈大小** - 多账户指令使用 `Box<>` 优化
3. **程序 ID** - 匹配目标环境期望的 ID
4. **判别符** - 使用自定义判别符匹配客户端
5. **Token 模块** - 根据环境选择 `token` 或 `token_interface`
6. **账户顺序** - **严格按照文档顺序定义账户**
7. **账户初始化** - 必要时使用 `init_if_needed`

---

## 参考资料

- [Anchor 官方文档](https://www.anchor-lang.com/)
- [Blueshift Anchor Escrow 挑战](https://learn.blueshift.gg/zh-CN/challenges/anchor-escrow)
- [Solana 栈大小限制](https://solana.com/docs/programs/faq#stack)
