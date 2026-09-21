# Bridge Cards Program

## Overview

The Bridge Cards Program is a Solana-based payment system that enables secure,
pull-based token transfers between users and card issuing merchants.

This program powers the [Bridge Cards](https://www.bridge.xyz/product/cards) product.

### How It Works

1. **Setup**: Bridge registers merchants and configures their payment destinations.
2. **User Approval**: Users grant spending authority to a PDA derived from their token
   account and a merchant ID. This is a one-time per-merchant-per-wallet step.
3. **Payments**: Bridge's authorized debitor initiates transfers from the user's token
   account to the merchant's destination account.

### Key Benefits

- **Seamless Payments**: Enable recurring charges without requiring a user signature for
  each transaction.
- **Pausable**: A global pause switch can halt all new-style transfers instantly.
- **Hierarchical Controls**: A layered role system (admin → governor → manager → debitor)
  limits the blast radius of any single compromised key.

## Deployments

| Network        | Account |
| -------------- | ------- |
| Mainnet (beta) | [`cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2`](https://explorer.solana.com/address/cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2) |
| Devnet         | [`cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2`](https://explorer.solana.com/address/cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2?cluster=devnet) |

## Audits

Bridge Cards was audited by [Zenith](https://zenith.security). You can find the report
[here](/audits/Bridge-Cards-Zenith-Audit-Report.pdf).

---

# Developer Documentation

## Architecture Overview

The program implements two coexisting systems:

- **Legacy system** — the original per-merchant role hierarchy, kept live while
  `debit_user` is still in production use.
- **Spender system** — a global role hierarchy with a pause switch, used by all new
  transfer instructions.

See [`SPENDER_MIGRATION.md`](./SPENDER_MIGRATION.md) for a full description of what
changed, why, and how to migrate callers off the legacy path.

### Roles

#### Spender system (new)

```
admin
 ├── governor
 │    ├── manager
 │    │    └── debitor  ← single global key that signs all new transfers
 │    └── manages destination allowlist
 └── pauser  ← can pause; cannot unpause (only admin/governor can unpause)
```

#### Legacy system

```
admin (BridgeCardsState)
 └── merchant manager  (one per merchant)
      └── debitor       (one per merchant)
```

### Program Derived Addresses (PDAs)

#### Spender system

| PDA | Seeds | Purpose |
|-----|-------|---------|
| `SpenderState` | `[b"spender_state"]` | Global roles and pause flag |
| `MerchantDelegateState` | `[b"merchant_delegate", merchant_id: [u8;32]]` | Signing authority for delegate-based transfers |
| `DelegateDestinationState` | `[b"merchant_destination", merchant_id: [u8;32], ata]` | Allowlisted destination for a merchant |

#### Legacy system

| PDA | Seeds | Purpose |
|-----|-------|---------|
| `BridgeCardsState` | `[b"state"]` | Legacy admin key |
| `MerchantManagerState` | `[b"merchant_manager", merchant_id_u64_le]` | Per-merchant manager key |
| `MerchantDebitorState` | `[b"merchant_debitor", merchant_id_u64_le, mint, debitor]` | Per-merchant debitor authorization |
| `MerchantDestinationState` | `[b"merchant_destination", merchant_id_u64_le, mint, ata]` | Per-merchant destination allowlist |
| `UserDelegateState` | `[b"user_delegate", merchant_id_u64_le, mint, user_ata]` | User's delegated signing authority |

### Transfer paths

| Instruction | Debitor auth | Destination check | User auth | Pausable |
|---|---|---|---|---|
| `debit_user` | Per-merchant `MerchantDebitorState` | Legacy `MerchantDestinationState` | `UserDelegateState` PDA | No |
| `transfer_using_legacy_delegate` | Global `SpenderState.debitor` | `DelegateDestinationState` | `UserDelegateState` PDA | Yes |
| `transfer_using_single_delegate` | Global `SpenderState.debitor` | `DelegateDestinationState` | SPL `approve` to `MerchantDelegateState` | Yes |
| `transfer_using_subscription_delegate` | Global `SpenderState.debitor` | `DelegateDestinationState` | Subscriptions program delegation | Yes |

`debit_user` is kept live for zero-downtime continuity. New integrations should use
`transfer_using_legacy_delegate` (for users who approved the legacy PDA) or
`transfer_using_single_delegate` / `transfer_using_subscription_delegate` (for new users).

---

## Client Integration

From a user's perspective, integration is unchanged: grant spending authority by
calling SPL `approve` with the `UserDelegateState` PDA as the delegate.

The PDA address is deterministic:

```
seeds = ["user_delegate", merchant_id_le_bytes8, mint, user_ata]
program = cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2
```

### TypeScript

```typescript
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
  clusterApiUrl,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createApproveInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { Buffer } from "buffer";
import { BN } from "@coral-xyz/anchor";

class BridgeSDK {
  public static readonly USER_DELEGATE_SEED = Buffer.from("user_delegate");
  constructor(private readonly programId: PublicKey) {}

  private formatAnchorNumber(number: BN): Buffer {
    return number.toArrayLike(Buffer, "le", 8);
  }

  findUserDelegatePDA(
    merchantId: BN,
    mintPubkey: PublicKey,
    userAta: PublicKey
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [
        BridgeSDK.USER_DELEGATE_SEED,
        this.formatAnchorNumber(merchantId),
        mintPubkey.toBuffer(),
        userAta.toBuffer(),
      ],
      this.programId
    );
  }
}

const PROGRAM_ID = new PublicKey("cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2");
const MINT_PUBKEY = new PublicKey("Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr"); // USDC
const MERCHANT_ID = new BN(1); // provided by Bridge
const MINT_DECIMALS = 6;
const APPROVAL_AMOUNT = BigInt(100 * 10 ** MINT_DECIMALS);

const connection = new Connection(clusterApiUrl("devnet"), "confirmed");

async function approveDelegate() {
  const userKeypair = Keypair.generate();
  const userAta = getAssociatedTokenAddressSync(MINT_PUBKEY, userKeypair.publicKey);

  const bridgeSdk = new BridgeSDK(PROGRAM_ID);
  const [delegatePda] = bridgeSdk.findUserDelegatePDA(MERCHANT_ID, MINT_PUBKEY, userAta);

  const approveInstruction = createApproveInstruction(
    userAta,
    delegatePda,
    userKeypair.publicKey,
    APPROVAL_AMOUNT,
    [],
    TOKEN_PROGRAM_ID
  );

  const transaction = new Transaction().add(approveInstruction);
  transaction.feePayer = userKeypair.publicKey;

  await sendAndConfirmTransaction(connection, transaction, [userKeypair]);
}

approveDelegate();
```

### Rust

```rust
use solana_sdk::{pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use solana_client::rpc_client::RpcClient;
use spl_token::instruction::approve;
use std::str::FromStr;

struct BridgeSDK {
    program_id: Pubkey,
}

impl BridgeSDK {
    const USER_DELEGATE_SEED: &'static [u8] = b"user_delegate";

    pub fn new(program_id: Pubkey) -> Self {
        Self { program_id }
    }

    pub fn find_user_delegate_pda(
        &self,
        merchant_id: u64,
        mint: &Pubkey,
        user_ata: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                Self::USER_DELEGATE_SEED,
                &merchant_id.to_le_bytes(),
                mint.as_ref(),
                user_ata.as_ref(),
            ],
            &self.program_id,
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let program_id = Pubkey::from_str("cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2")?;
    let mint = Pubkey::from_str("Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr")?; // USDC
    let merchant_id: u64 = 1; // provided by Bridge

    let user_keypair = Keypair::new();
    let user_ata = spl_associated_token_account::get_associated_token_address(
        &user_keypair.pubkey(),
        &mint,
    );

    let sdk = BridgeSDK::new(program_id);
    let (delegate_pda, _) = sdk.find_user_delegate_pda(merchant_id, &mint, &user_ata);

    let rpc = RpcClient::new("https://api.devnet.solana.com");
    let approve_ix = approve(
        &spl_token::ID,
        &user_ata,
        &delegate_pda,
        &user_keypair.pubkey(),
        &[],
        100 * 10u64.pow(6),
    )?;

    let tx = Transaction::new_signed_with_payer(
        &[approve_ix],
        Some(&user_keypair.pubkey()),
        &[&user_keypair],
        rpc.get_latest_blockhash()?,
    );

    let sig = rpc.send_and_confirm_transaction(&tx)?;
    println!("Signature: {}", sig);
    Ok(())
}
```

## Audits

Bridge Cards was audited by [Zenith](https://zenith.security). You can find the report
[here](/audits/Bridge-Cards-Zenith-Audit-Report.pdf).
