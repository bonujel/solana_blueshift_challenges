# Pinocchio Vault 开发笔记

> 记录项目开发过程中遇到的问题和解决方案

## 项目概述

基于 [Blueshift Pinocchio Vault Challenge](https://learn.blueshift.gg/zh-CN/challenges/pinocchio-vault) 的 Solana 程序实现。

**功能：**
- Deposit: 将 lamports 存入 PDA vault
- Withdraw: 从 PDA vault 提取所有 lamports（仅限原存款人）

---

## 遇到的问题

### 1. 依赖版本冲突

**现象：**
```
error[E0308]: mismatched types
note: there are multiple different versions of crate `pinocchio` in the dependency graph
```

**错误配置：**
```toml
pinocchio = "0.8"
pinocchio-system = "0.4"
```

**根因分析：**
- `pinocchio-system 0.4` 内部依赖 `pinocchio 0.9`
- Cargo 同时引入了 `pinocchio 0.8` 和 `pinocchio 0.9`
- 两个版本的类型（如 `AccountInfo`、`ProgramError`）是**完全不同的类型**
- 即使结构定义一模一样，Rust 类型系统认为它们不兼容

**解决方案：**
```toml
pinocchio = "0.9"  # 与 pinocchio-system 的依赖保持一致
pinocchio-system = "0.4"
```

**排查工具：**
```bash
cargo tree  # 查看完整依赖图
cargo tree -d  # 只显示重复依赖
```

---

### 2. Signer API 差异

**现象：**
```
error[E0308]: mismatched types
expected `Signer<'_, '_>`, found `&[&[u8]; 3]`
```

**错误写法（标准 Solana SDK 风格）：**
```rust
let signer_seeds = [
    VAULT_SEED,
    owner.key().as_ref(),
    &bump_bytes,
];
transfer.invoke_signed(&[&signer_seeds])?;
```

**正确写法（Pinocchio 风格）：**
```rust
use pinocchio::{instruction::Signer, seeds};

let signer_seeds = seeds!(VAULT_SEED, self.owner.key().as_ref(), &bump_bytes);
let signer = Signer::from(&signer_seeds);
transfer.invoke_signed(&[signer])?;
```

**Pinocchio vs 标准 SDK 对比：**

| 特性 | 标准 Solana SDK | Pinocchio |
|------|----------------|-----------|
| Signer 类型 | `&[&[&[u8]]]` | `&[Signer]` |
| 构建方式 | 手动嵌套数组 | `seeds!` 宏 + `Signer::from` |
| 类型安全 | 弱（编译时难以检查） | 强（类型封装） |

---

### 3. cfg 条件警告

**现象：**
```
warning: unexpected `cfg` condition value: `solana`
```

**原因：**
- Pinocchio 宏内部使用 `#[cfg(target_os = "solana")]`
- 本地 `cargo build` 目标是 Linux/macOS，不是 Solana VM
- 使用 `cargo build-sbf` 构建时目标正确，无此警告

**处理：**
- 开发时可忽略，不影响最终 `.so` 文件
- 或添加到 `Cargo.toml` 抑制：
  ```toml
  [lints.rust]
  unexpected_cfgs = { level = "warn", check-cfg = ['cfg(target_os, values("solana"))'] }
  ```

---

## 关键实现细节

### PDA 派生

```rust
pub const VAULT_SEED: &[u8] = b"vault";

// 派生 vault 地址
let (vault_pda, bump) = find_program_address(
    &[VAULT_SEED, owner.key().as_ref()],
    &program_id,
);
```

### 指令 Discriminator

| 指令 | Discriminator | 数据 |
|------|--------------|------|
| Deposit | `0` | 8 bytes (u64 amount, little-endian) |
| Withdraw | `1` | 无 |

### 账户顺序

```
[0] owner         - 签名者，资金来源/目标
[1] vault         - PDA，资金存储
[2] system_program - 用于 CPI 转账
```

---

## 构建和部署

```bash
# 本地开发构建（会有 cfg 警告）
cargo build

# 构建 Solana BPF 程序
cargo build-sbf

# 输出文件
target/deploy/blueshift_vault.so
```

---

## 经验总结

1. **依赖管理**
   - 优先使用 `cargo add` 自动选择兼容版本
   - 遇到类型不匹配时，先检查是否有版本冲突

2. **框架学习**
   - 查阅框架源码（`~/.cargo/registry/src/`）了解 API
   - 不要用其他框架的经验假设 API 用法

3. **调试技巧**
   - `cargo tree` 是排查依赖问题的利器
   - 错误信息中的 "multiple versions" 是关键提示
