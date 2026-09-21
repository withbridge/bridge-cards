# Spender Migration

This document describes the upgrade of the Bridge Cards Solana program
(`cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2`) to adopt the Spender Program's
architecture. The program ID is unchanged; the binary is deployed as an upgrade.

---

## Why

The original Bridge Cards program had per-merchant access control (each merchant had
its own manager and debitor), velocity controls on transfers, and no pause mechanism.
The Spender architecture introduces a cleaner global role hierarchy, removes velocity
controls, and adds a pause switch. This migration adopts that architecture while
keeping `debit_user` running without downtime.

---

## Access control: old vs. new

### Legacy (still active for `debit_user`)

```
BridgeCardsState.admin
  └── MerchantManagerState (per-merchant)
        └── MerchantDebitorState (per-merchant)
              └── debit_user
```

Each merchant had its own manager and debitor PDAs. Velocity limits (per-transfer
and per-period caps) were enforced per user delegate.

### New (SpenderState)

```
SpenderState.admin
  ├── SpenderState.governor
  │     ├── SpenderState.manager
  │     │     └── SpenderState.debitor  ← single global debitor
  │     └── destination allowlist management
  └── SpenderState.pauser  ← can pause but cannot unpause
```

One global debitor signs all new-style transfers. There are no velocity controls.
Pausing is split: the pauser role can pause, but only admin or governor can unpause.

Both systems coexist in the same binary. The legacy path has no awareness of
`SpenderState`; the new path has no awareness of `BridgeCardsState`.

---

## Instructions: what came from where

### Kept from the original Bridge Cards program (`instructions/legacy/`)

These instructions are still live for production continuity but should not be used
for new integrations.

| Instruction | Notes |
|---|---|
| `initialize` | Created `BridgeCardsState` at `b"state"`. Already run in production. |
| `add_or_update_merchant_manager` | Sets per-merchant manager. |
| `add_or_update_merchant_debitor` | Sets per-merchant debitor. |
| `add_or_update_merchant_destination` | Allowlists a destination using the legacy seed layout (`[seed, merchant_id_u64, mint, ata]`). |
| `add_or_update_user_delegate` | Sets up a user delegate with velocity limits. Velocity fields are written but no longer enforced. |
| `update_admin` | Rotates `BridgeCardsState.admin`. |
| `close_account` | Closes any PDA; checks `BridgeCardsState.admin`. |
| `debit_user` | **See below.** |

### Modified from Bridge Cards: `debit_user`

`debit_user` keeps its exact original account list and arguments so that existing
callers experience zero downtime. The only behavioral change is that velocity checks
(`validate_debit_and_update`) have been removed. Stale velocity values in
`UserDelegateState` accounts are left in place and ignored.

`debit_user` is intentionally **not pausable**. A future migration will remove it
entirely once all callers have switched to `transfer_using_legacy_delegate`.

### Ported from the Spender program

| Instruction | Notes |
|---|---|
| `initialize_spender_state` | Creates `SpenderState` at seed `b"spender_state"` (different from `BridgeCardsState` at `b"state"`). Requires program keypair signature in production. |
| `update_governor` | Sets `SpenderState.governor`. Called by admin. |
| `update_manager` | Sets `SpenderState.manager`. Called by governor. |
| `update_debitor` | Sets `SpenderState.debitor`. Called by manager. |
| `update_pauser` | Sets `SpenderState.pauser`. Called by admin. |
| `update_spender_admin` | Rotates `SpenderState.admin`. Both current and new admin must sign. |
| `update_paused` | Pauses or unpauses all new-style transfers. **Modified from Spender**: pauser/admin/governor can pause; only admin or governor can unpause. |
| `setup_merchant_delegate` | Creates a `MerchantDelegateState` PDA for a merchant and optionally bulk-initializes destinations. Called by manager. |
| `add_delegate_destination` | Allowlists a destination for a merchant using the new seed layout (`[seed, merchant_id_bytes32, ata]`). Called by governor. |
| `close_delegate_destination` | Removes a destination from the allowlist. Called by governor. |
| `transfer_using_single_delegate` | Transfers via SPL `approve` delegation. Pausable. |
| `transfer_using_subscription_delegate` | Transfers via a fixed or recurring subscriptions delegation. Pausable. |

### Net-new: `transfer_using_legacy_delegate`

This instruction is the intended long-term successor to `debit_user` for users who
onboarded via the legacy PDA approval path (i.e. they called SPL `approve` with the
old `UserDelegateState` PDA as the authority).

**Why it's needed**: users who approved the old PDA — derived from a `u64` merchant
ID — cannot be migrated to SPL `approve` without their action. This instruction lets
us serve those users through the new access control system without requiring them to
re-approve.

**How it works**:

```
Arguments
  program_id:  [u8; 32]   new-style merchant key (for destination validation)
  merchant_id: u64         legacy merchant key (for user delegate PDA derivation)
  amount:      u64

Access control
  debitor     must be SpenderState.debitor (global)
  destination must be in DelegateDestinationState allowlist (keyed by program_id)
  paused      respects SpenderState.paused

User delegate PDA
  seeds: [b"user_delegate", merchant_id_le_bytes, mint, user_ata]
  init_if_needed — new users do not need to call add_or_update_user_delegate first
  velocity fields initialized to zero and ignored
```

Before `transfer_using_legacy_delegate` can be used for a given merchant, the
corresponding `u64` merchant ID must be registered as a `[u8; 32]` program ID via
`setup_merchant_delegate`. See "Migration steps" below.

---

## State accounts

| Account | Seed | Origin | Used by |
|---|---|---|---|
| `BridgeCardsState` | `b"state"` | Legacy | `debit_user` path admin instructions |
| `UserDelegateState` | `[b"user_delegate", merchant_id_u64_le, mint, ata]` | Legacy | `debit_user`, `transfer_using_legacy_delegate` |
| `MerchantManagerState` | `[b"merchant_manager", merchant_id_u64_le]` | Legacy | `debit_user` path |
| `MerchantDebitorState` | `[b"merchant_debitor", merchant_id_u64_le, mint, debitor]` | Legacy | `debit_user` path |
| `MerchantDestinationState` | `[b"merchant_destination", merchant_id_u64_le, mint, ata]` | Legacy | `debit_user` path |
| `SpenderState` | `b"spender_state"` | Spender | All new-style instructions |
| `MerchantDelegateState` | `[b"merchant_delegate", merchant_id_bytes32]` | Spender | New-style transfers |
| `DelegateDestinationState` | `[b"merchant_destination", merchant_id_bytes32, ata]` | Spender | New-style transfers |

> Note: `DelegateDestinationState` and the legacy `MerchantDestinationState` share
> the `b"merchant_destination"` prefix but have different seed lengths and different
> on-chain layouts, so their PDA addresses never collide.

---

## Transfer paths side by side

| | `debit_user` | `transfer_using_legacy_delegate` | `transfer_using_single_delegate` |
|---|---|---|---|
| Debitor | Per-merchant `MerchantDebitorState` | Global `SpenderState.debitor` | Global `SpenderState.debitor` |
| Destination validation | Legacy `MerchantDestinationState` | New `DelegateDestinationState` | New `DelegateDestinationState` |
| User approval PDA | `UserDelegateState` (u64 merchant) | `UserDelegateState` (u64 merchant) | User-held SPL approval |
| Auto-init user PDA | No | Yes (`init_if_needed`) | N/A |
| Velocity controls | No (removed) | No | No |
| Pausable | No | Yes | Yes |
| Status | Live, will be removed | Rollout target | Live |

---

## Migration steps

### One-time setup (per environment)

```bash
# 1. Initialize SpenderState
anchor call initialize_spender_state \
  --args admin governor manager debitor pauser

# 2. For each legacy merchant, register its u64 ID as a [u8; 32] program_id.
#    Encode the u64 as little-endian bytes zero-padded to 32 bytes.
anchor call setup_merchant_delegate --args <merchant_id_bytes32>

# 3. Allowlist destination accounts for each merchant under the new system.
anchor call add_delegate_destination --args <merchant_id_bytes32>
```

### Migrating callers off `debit_user`

For each caller currently using `debit_user`:

1. Ensure the merchant's `u64` ID has been registered as a `[u8; 32]` program ID
   (step 2 above).
2. Switch the caller to `transfer_using_legacy_delegate`, passing both the
   `[u8; 32]` program ID and the original `u64` merchant ID. The `UserDelegateState`
   will be created automatically on the first call if it doesn't already exist.
3. Once all callers are migrated, `debit_user` can be removed in a future upgrade.

---

## Anchor version upgrade

This migration also upgrades the program from Anchor 0.31 to Anchor 1.0. The on-chain
ABI (instruction discriminators and account discriminators) is identical between the
two versions — both derive discriminators via `SHA256("global:<name>")[..8]` and
`SHA256("account:<name>")[..8]` respectively. Existing clients require no changes.

The bridge-cards program was not deployed with an on-chain IDL account, so no IDL
cleanup is needed before deploying the upgraded binary.
