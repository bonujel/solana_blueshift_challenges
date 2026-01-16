# Solana Blueshift Challenges

Completed challenges from [Blueshift Learn](https://learn.blueshift.gg/) - A hands-on Solana development learning platform.

## Challenges

### 1. SPL Token Minting ✅

**Path:** `SPL_token/`

Mint an SPL token in a single transaction using Web3.js and the SPL Token library.

**Objectives:**
- Create an SPL mint account
- Initialize the mint with 6 decimals
- Create an associated token account (ATA)
- Mint 21,000,000 tokens

**Tech Stack:** TypeScript, @solana/web3.js, @solana/spl-token

```bash
cd SPL_token
npm install
npm start
```

---

### 2. Anchor Vault ✅

**Path:** `blueshift_anchor_vault/`

Build a simple lamport vault on Solana using the Anchor framework.

**Objectives:**
- Implement `deposit` function - accept lamports into a PDA vault
- Implement `withdraw` function - allow vault owners to retrieve funds
- Use PDA signing for secure withdrawals

**Tech Stack:** Rust, Anchor Framework

**Key Concepts:**
- Program-Derived Addresses (PDA)
- Cross-Program Invocations (CPI)
- Anchor account constraints

```bash
cd blueshift_anchor_vault
anchor build
# Output: target/deploy/blueshift_anchor_vault.so
```

---

## Environment Requirements

- Node.js >= 18
- Rust & Cargo
- Solana CLI
- Anchor CLI

## License

MIT
