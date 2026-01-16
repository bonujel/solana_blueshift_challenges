/** Challenge: Mint an SPL Token
 *
 * In this challenge, you will create an SPL token!
 *
 * Goal:
 *   Mint an SPL token in a single transaction using Web3.js and the SPL Token library.
 *
 * Objectives:
 *   1. Create an SPL mint account.
 *   2. Initialize the mint with 6 decimals and your public key (feePayer) as the mint and freeze authorities.
 *   3. Create an associated token account for your public key (feePayer) to hold the minted tokens.
 *   4. Mint 21,000,000 tokens to your associated token account.
 *   5. Sign and send the transaction.
 */

import {
  Keypair,
  Connection,
  sendAndConfirmTransaction,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";

import {
  createAssociatedTokenAccountInstruction,
  createInitializeMint2Instruction,
  createMintToInstruction,
  createMintToCheckedInstruction,
  MINT_SIZE,
  getMinimumBalanceForRentExemptMint,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

import * as fs from "fs";
import * as path from "path";

// ============================================================
// 本地环境配置 (用于本地开发调试)
// 提交到 Blueshift 时替换为环境变量版本
// ============================================================
function loadLocalWallet(): Keypair {
  const walletPath = path.join(
    process.env.HOME || "",
    ".config/solana/id.json"
  );
  const secretKey = JSON.parse(fs.readFileSync(walletPath, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(secretKey));
}

const feePayer = loadLocalWallet();
const connection = new Connection("https://api.devnet.solana.com", "confirmed");

// ============================================================
// Blueshift 版本 (提交时取消注释以下代码，注释掉上面的本地版本)
// ============================================================
// import bs58 from "bs58";
// const feePayer = Keypair.fromSecretKey(bs58.decode(process.env.SECRET!));
// const connection = new Connection(process.env.RPC_ENDPOINT!, "confirmed");

// Entry point of your TypeScript code (we will call this)
async function main() {
  try {
    console.log("🚀 SPL Token Minting Challenge\n");
    console.log("👛 Fee Payer:", feePayer.publicKey.toBase58());

    // Generate a new keypair for the mint account
    const mint = Keypair.generate();
    console.log("🪙 Mint Address:", mint.publicKey.toBase58());

    const mintRent = await getMinimumBalanceForRentExemptMint(connection);
    console.log("💰 Mint Rent:", mintRent, "lamports\n");

    // ============================================================
    // START HERE - 完成以下四个任务
    // ============================================================

    // Task 1: Create the mint account
    // 使用 SystemProgram.createAccount() 在链上分配空间
    const createAccountIx = SystemProgram.createAccount({
      fromPubkey: feePayer.publicKey,    // 支付租金的账户
      newAccountPubkey: mint.publicKey,  // 新 Mint 账户的公钥
      lamports: mintRent,                // 租金金额
      space: MINT_SIZE,                  // Mint 账户固定大小 (82 bytes)
      programId: TOKEN_PROGRAM_ID,       // 账户所有者 = Token Program
    });

    // Task 2: Initialize the mint account
    // 设置 decimals=6, mint/freeze authority 都指向 feePayer
    const initializeMintIx = createInitializeMint2Instruction(
      mint.publicKey,       // mint 账户
      6,                    // decimals
      feePayer.publicKey,   // mint authority
      feePayer.publicKey,   // freeze authority
      TOKEN_PROGRAM_ID      // program id
    );

    // Task 3: Create the associated token account
    // 先计算 ATA 地址，再创建指令
    const associatedTokenAccount = getAssociatedTokenAddressSync(
      mint.publicKey,       // mint
      feePayer.publicKey    // owner
    );

    const createAssociatedTokenAccountIx = createAssociatedTokenAccountInstruction(
      feePayer.publicKey,       // payer
      associatedTokenAccount,   // ata 地址
      feePayer.publicKey,       // owner
      mint.publicKey            // mint
    );

    // Task 4: Mint 21,000,000 tokens to the associated token account
    // 注意: 实际数量 = 21_000_000 * 10^6 (因为 decimals = 6)
    const mintAmount = BigInt(21_000_000) * BigInt(10 ** 6);

    const mintToCheckedIx = createMintToCheckedInstruction(
      mint.publicKey,           // mint
      associatedTokenAccount,   // destination
      feePayer.publicKey,       // mint authority
      mintAmount,               // amount (带精度)
      6                         // decimals (用于校验)
    );

    // ============================================================
    // 构建并发送交易
    // ============================================================

    const recentBlockhash = await connection.getLatestBlockhash();

    const transaction = new Transaction({
      feePayer: feePayer.publicKey,
      blockhash: recentBlockhash.blockhash,
      lastValidBlockHeight: recentBlockhash.lastValidBlockHeight,
    }).add(
      createAccountIx,
      initializeMintIx,
      createAssociatedTokenAccountIx,
      mintToCheckedIx
    );

    // Task 5: 签名者列表
    // feePayer: 支付交易费 + 创建账户的租金 + mint authority
    // mint: 新账户需要签名确认其公钥
    const transactionSignature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [feePayer, mint]
    );

    console.log("\n✅ Success!");
    console.log("🪙 Mint Address:", mint.publicKey.toBase58());
    console.log("📬 ATA Address:", associatedTokenAccount.toBase58());
    console.log("🔗 Transaction:", transactionSignature);
  } catch (error) {
    console.error(`Oops, something went wrong: ${error}`);
  }
}

main();
