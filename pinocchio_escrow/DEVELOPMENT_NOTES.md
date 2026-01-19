# Pinocchio Escrow 开发笔记

## 项目概述

使用 Pinocchio（轻量级 `#![no_std]` Solana 框架）实现 Escrow 程序，完成 learn.blueshift.gg 挑战。

### 三个指令
| 指令 | Discriminator | 功能 | 账户数量 |
|------|---------------|------|----------|
| Make | 0 | 创建托管报价 | 9 |
| Take | 1 | 接受托管报价 | 12 |
| Refund | 2 | 取消托管报价 | 7 |

---

## 遇到的问题与解决方案

### 问题 1：依赖包名称错误

**错误信息**：找不到 `pinocchio-associated-token` crate

**解决方案**：正确的包名是 `pinocchio-associated-token-account`

```toml
[dependencies]
pinocchio-associated-token-account = "0.4"
```

---

### 问题 2：API 版本不兼容

**错误信息**：pinocchio 0.10 的 API 与挑战环境不匹配

**解决方案**：降级到 pinocchio 0.9 系列

```toml
[dependencies]
pinocchio = "0.9"
pinocchio-token = "0.4"
pinocchio-system = "0.3"
pinocchio-associated-token-account = "0.4"
```

---

### 问题 3：缺少 Sysvar trait 导入

**错误信息**：`Rent::get()` 无法调用

**解决方案**：添加 Sysvar trait 导入

```rust
use pinocchio::sysvars::Sysvar;

// 然后可以调用
let rent = pinocchio::sysvars::rent::Rent::get()?;
```

---

### 问题 4：InitializeAccount3 参数类型错误

**错误信息**：`owner` 字段期望 `&Pubkey`，不是 `&AccountInfo`

**解决方案**：使用 `.key()` 方法获取 `&Pubkey`

```rust
InitializeAccount3 {
    account: ata,
    mint,
    owner: owner.key(),  // 正确：&Pubkey
}
.invoke()?;
```

---

### 问题 5：Seed 类型不存在

**错误信息**：`pinocchio::seed::Seed` 在 0.9 版本中不存在

**解决方案**：使用 `seeds!` 宏和 `Signer::from`

```rust
use pinocchio::{instruction::Signer, seeds};

let seed_bytes = escrow.seed.to_le_bytes();
let bump_bytes = escrow.bump;
let signer_seeds = seeds!(
    ESCROW_SEED,
    maker_key.as_ref(),
    seed_bytes.as_ref(),
    bump_bytes.as_ref()
);
let signer = Signer::from(&signer_seeds);

// 使用签名调用
Transfer { ... }.invoke_signed(&[signer.clone()])?;
```

---

### 问题 6：Signer 权限提升错误

**错误信息**：`Cross-program invocation with unauthorized signer or writable account`

**原因分析**：
- 尝试用 `CreateAccount` 创建 vault（ATA）
- vault 是 ATA 程序的 PDA，我们的程序没有签名权限

**解决方案**：使用 ATA 程序的 CPI 指令

```rust
use pinocchio_associated_token_account::instructions::Create;

// Make 指令中创建 vault
Create {
    funding_account: accounts.maker,
    account: accounts.vault,
    wallet: accounts.escrow,
    mint: accounts.mint_a,
    system_program: accounts.system_program,
    token_program: accounts.token_program,
}
.invoke()?;
```

---

### 问题 7：结构体字段名错误

**错误信息**：`struct Create has no field named associated_account`

**解决方案**：字段名是 `account`，不是 `associated_account`

```rust
// 错误
Create {
    associated_account: accounts.vault,  // ❌
    ...
}

// 正确
Create {
    account: accounts.vault,  // ✅
    ...
}
```

---

### 问题 8：Refund 账户数量错误

**错误信息**：`Invalid account owner`

**原因分析**：添加了文档中不存在的 `associated_token_program` 账户，导致账户解析错位

**解决方案**：严格按照文档，Refund 只需要 7 个账户

```rust
let [maker, escrow, mint_a, vault, maker_ata_a, system_program, token_program, _remaining @ ..] =
    accounts
```

---

### 问题 9：maker_ata_a 可能未初始化

**错误信息**：`Invalid account owner`（ATA 验证时）

**原因分析**：
- 测试环境传入的 `maker_ata_a` 可能尚未初始化
- 未初始化的账户 owner 是 system program，不是 token program
- `AssociatedTokenAccount::check` 验证 owner 失败

**解决方案**：在验证前使用 `CreateIdempotent` 自动创建/初始化

```rust
use pinocchio_associated_token_account::instructions::CreateIdempotent;

// 先确保 ATA 存在
CreateIdempotent {
    funding_account: maker,
    account: maker_ata_a,
    wallet: maker,
    mint: mint_a,
    system_program,
    token_program,
}
.invoke()?;

// 然后再验证
AssociatedTokenAccount::check(maker_ata_a, maker, mint_a, token_program)?;
```

---

## 账户顺序参考

### Make 指令（9 账户）
```
0. maker              - 签名者，可变
1. escrow             - PDA，可变
2. mint_a             - Token A 的 Mint
3. mint_b             - Token B 的 Mint
4. maker_ata_a        - Maker 的 Token A ATA，可变
5. vault              - Escrow 的 Token A ATA，可变
6. system_program     - 系统程序
7. token_program      - Token 程序
8. associated_token_program - ATA 程序
```

### Take 指令（12 账户）
```
0. taker              - 签名者，可变
1. maker              - 可变
2. escrow             - PDA，可变
3. mint_a             - Token A 的 Mint
4. mint_b             - Token B 的 Mint
5. vault              - Escrow 的 Token A ATA，可变
6. taker_ata_a        - Taker 的 Token A ATA，可变
7. taker_ata_b        - Taker 的 Token B ATA，可变
8. maker_ata_b        - Maker 的 Token B ATA，可变
9. system_program     - 系统程序
10. token_program     - Token 程序
11. associated_token_program - ATA 程序
```

### Refund 指令（7 账户）
```
0. maker              - 签名者，可变
1. escrow             - PDA，可变
2. mint_a             - Token A 的 Mint
3. vault              - Escrow 的 Token A ATA，可变
4. maker_ata_a        - Maker 的 Token A ATA，可变
5. system_program     - 系统程序
6. token_program      - Token 程序
```

---

## 关键经验总结

| 类别 | 经验 |
|------|------|
| **依赖管理** | 仔细核对 crate 名称和版本兼容性 |
| **账户顺序** | 必须与挑战平台期望的顺序完全一致 |
| **账户数量** | 严格按照文档，不多不少 |
| **ATA 创建** | 使用 ATA 程序 CPI（Create/CreateIdempotent），不能用 CreateAccount |
| **防御性编程** | 对可能未初始化的账户使用 `CreateIdempotent` |
| **错误定位** | 通过 compute units 消耗量判断错误发生位置 |
| **字段名称** | 查阅 crate 源码确认结构体字段名 |

---

## 文件结构

```
pinocchio_escrow/
├── Cargo.toml
├── DEVELOPMENT_NOTES.md    # 本文档
└── src/
    ├── lib.rs              # 入口点 + 指令路由
    ├── state.rs            # Escrow 账户结构 (113 bytes)
    ├── helpers.rs          # 账户验证辅助函数
    └── instructions/
        ├── mod.rs
        ├── make.rs         # 创建托管报价
        ├── take.rs         # 接受托管报价
        └── refund.rs       # 取消托管报价
```

---

## 构建命令

```bash
# 构建 BPF 程序
cargo build-sbf

# 输出文件位置
target/deploy/pinocchio_escrow.so
```

---

## 参考资源

- [Pinocchio GitHub](https://github.com/febo/pinocchio)
- [Blueshift Learn](https://learn.blueshift.gg)
- [Solana Program Library](https://github.com/solana-labs/solana-program-library)
