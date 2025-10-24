# Manual-Claim JIT Example - Quick Start

This example demonstrates holding HTLCs indefinitely to test timeout behavior.

## Setup

1. **Copy environment configuration:**

   ```bash
   cp .env.example .env
   ```

2. **Edit `.env`** with your LSP details (already configured for Megalith):

   ```bash
   LSP_PUBKEY=034066e29e402d9cf55af1ae1026cc5adf92eed1e0e421785442f53717ad1453b0
   LSP_ADDRESS=64.23.159.177:9735
   NETWORK=bitcoin
   ESPLORA_API_URL=https://blockstream.info/api
   ```

3. **For faster HTLC timeout testing**, set a shorter expiry:
   ```bash
   # Add to .env for faster timeout:
   INVOICE_EXPIRY_SECS=300  # 5 minutes (instead of default 1 hour)
   ```

## Running the Example

```bash
cargo run --example manual_claim_jit
```

## What It Does

1. ✅ Starts an LDK node with LSPS2 liquidity support
2. 📄 Creates a **manual-claim JIT invoice** for 25,000 sats
3. 🔗 When you pay the invoice, the LSP opens a JIT channel
4. ⏸️ **Holds the HTLC indefinitely** (does NOT claim or fail)
5. ⏰ Waits for HTLC timeout or manual shutdown

## Expected Behavior

### Normal Flow (if you claim):

- Payment arrives → `PaymentClaimable` event → Call `claim_for_hash()` → Payment settles

### THIS Example (holding):

- Payment arrives → `PaymentClaimable` event → **DO NOTHING** → HTLC remains pending
- Eventually:
  - **Option 1**: Press Enter to shutdown (auto-fails the payment)
  - **Option 2**: Wait for CLTV timeout (LSP may force-close channel)
  - **Option 3**: Manually modify code to claim/fail

## Understanding HTLC Timeouts

The HTLC timeout is controlled by `min_final_cltv_expiry_delta` in the invoice:

- **Default**: 80 blocks (~13 hours on Bitcoin mainnet)
- **Current setting**: Uses LDK default + 2 blocks for LSPS2

### Block Times by Network:

- **Bitcoin mainnet**: ~10 minutes per block
  - 80 blocks = ~13 hours
  - 144 blocks = ~24 hours
- **Testnet**: ~10 minutes per block (similar to mainnet)
- **Regtest**: Blocks mined on-demand (instant)

### To Speed Up Testing:

The invoice expiry (`INVOICE_EXPIRY_SECS`) controls when the invoice becomes invalid, but the **HTLC timeout** is determined by the CLTV delta which is currently hardcoded.

For the **fastest testing**, use:

1. **Regtest network**: Blocks are instant, you control when they're mined
2. **Short invoice expiry**: Set `INVOICE_EXPIRY_SECS=300` (5 minutes)

⚠️ **Note**: The actual HTLC CLTV timeout is hardcoded in `ldk-node` and cannot currently be configured via environment variables. To change it, you would need to modify:

- `src/liquidity.rs` line 1202: `let min_final_cltv_expiry_delta = MIN_FINAL_CLTV_EXPIRY_DELTA + 2;`
- Change to something like: `let min_final_cltv_expiry_delta = 10;` for faster testing

## Output Example

```
=== Manual-Claim JIT Channel Test (Hold HTLC Indefinitely) ===

Node started successfully!
Node ID: 03abc123...
Logs: tmp_manual_claim/manual_claim_jit.log

Creating manual-claim JIT invoice:
  Amount: 25000000 msats (25000 sats)
  Expiry: 3600 seconds (~60 minutes)

=== MANUAL-CLAIM JIT INVOICE ===
Payment Hash: PaymentHash([...])
Invoice:
lnbc250u1...

================================

INSTRUCTIONS:
1. Pay this invoice from another node
2. The LSP will open a JIT channel
3. When payment arrives, you'll see a PaymentClaimable event
4. This example will HOLD the HTLC (not claim it immediately)
5. You can then manually claim or fail the payment

Monitoring for payment events...
Press Enter to shutdown

📢 Channel pending: ChannelId([...]) with Some(PublicKey(...))
✅ Channel ready: ChannelId([...]) with Some(PublicKey(...))

🎯 PAYMENT CLAIMABLE EVENT RECEIVED!
   Payment ID: PaymentId(...)
   Payment Hash: PaymentHash(...)
   Amount: 25000000 msats (25000 sats)
   Claim Deadline: Some(80) blocks

⏸️  HOLDING HTLC INDEFINITELY (not claiming or failing)
   This demonstrates holding payment in limbo
   The HTLC will remain pending until:
   - You press Enter to shutdown (will auto-fail)
   - The CLTV deadline expires (LSP may force-close)
   - You manually claim/fail via code modification
```

## Modifying the Behavior

To **claim** the payment instead of holding, modify the code at line ~150:

```rust
// Uncomment this line:
node.bolt11_payment().claim_for_hash(payment_hash).expect("Failed to claim");
```

To **fail** immediately (instead of holding):

```rust
// Uncomment this line:
node.bolt11_payment().fail_for_hash(payment_hash).expect("Failed to fail");
```

## Troubleshooting

### "Failed to create manual-claim JIT invoice"

- Check your LSP configuration in `.env`
- Ensure LSP is reachable at the specified address
- Check logs in `tmp_manual_claim/manual_claim_jit.log`

### HTLC timeout too slow

- Use `NETWORK=regtest` for instant block mining
- Or modify the CLTV delta in `src/liquidity.rs` (requires recompile)

### LSP force-closes channel

- This is expected if you hold the HTLC past the CLTV deadline
- The LSP protects itself by force-closing to recover funds

## Related Documentation

- [MANUAL_CLAIM_JIT.md](MANUAL_CLAIM_JIT.md) - Full API documentation
- LDK Documentation: https://docs.rs/lightning/latest/lightning/
- LSPS2 Spec: https://github.com/BitcoinAndLightningLayerSpecs/lsp/blob/main/LSPS2/README.md
