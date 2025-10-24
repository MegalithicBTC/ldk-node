# Manual-Claim JIT Channel Feature

## Overview

This feature adds manual-claim support for Just-In-Time (JIT) channel invoices created via LSPS2. This allows applications to explicitly control when incoming payments are claimed, enabling use cases such as:

- Testing hold invoice behavior
- Implementing custom payment acceptance logic
- Demonstrating HTLC timeout scenarios
- Building applications that need to verify conditions before accepting payments

## New API Methods

### Bolt11Payment

Two new public methods have been added to `ldk_node::payment::bolt11::Bolt11Payment`:

#### 1. `receive_via_jit_channel_for_hash`

```rust
pub fn receive_via_jit_channel_for_hash(
    &self,
    amount_msat: u64,
    description: &Bolt11InvoiceDescription,
    expiry_secs: u32,
    max_total_lsp_fee_limit_msat: Option<u64>,
) -> Result<(PaymentHash, Bolt11Invoice), Error>
```

Creates a fixed-amount JIT channel invoice that requires manual claim.

- Returns both the payment hash and the invoice
- When paid, emits `Event::PaymentClaimable`
- **Must** call `claim_for_hash(payment_hash)` or `fail_for_hash(payment_hash)` to settle

#### 2. `receive_variable_amount_via_jit_channel_for_hash`

```rust
pub fn receive_variable_amount_via_jit_channel_for_hash(
    &self,
    description: &Bolt11InvoiceDescription,
    expiry_secs: u32,
    max_proportional_lsp_fee_limit_ppm_msat: Option<u64>,
) -> Result<(PaymentHash, Bolt11Invoice), Error>
```

Creates a zero-amount (variable) JIT channel invoice that requires manual claim.

- Returns both the payment hash and the invoice
- Amount is determined by the payer
- When paid, emits `Event::PaymentClaimable`
- **Must** call `claim_for_hash(payment_hash)` or `fail_for_hash(payment_hash)` to settle

## Implementation Details

### Modified Files

1. **src/payment/bolt11.rs**

   - Added `receive_via_jit_channel_for_hash()` method
   - Added `receive_variable_amount_via_jit_channel_for_hash()` method
   - Added `receive_via_jit_channel_inner_for_hash()` helper method
   - Updated `receive_via_jit_channel_inner()` to accept optional manual claim parameter

2. **src/liquidity.rs**
   - Added `lsps2_receive_to_jit_channel_for_hash()` method
   - Added `lsps2_receive_variable_amount_to_jit_channel_for_hash()` method
   - Refactored `lsps2_receive_to_jit_channel_inner()` to support manual claim
   - Refactored `lsps2_receive_variable_amount_to_jit_channel_inner()` to support manual claim
   - Updated `lsps2_create_jit_invoice()` to handle both auto-claim and manual-claim paths

### Key Design Decisions

1. **Payment Hash Generation**: The implementation generates a random payment hash using SHA256 hashing of timestamp-based entropy. In production use, applications should provide their own payment hashes derived from proper preimages.

2. **No Preimage Storage**: Manual-claim invoices do not store preimages in the payment store, since the application is responsible for claiming with the correct preimage via `claim_for_hash()`.

3. **LSPS2 Compatibility**: The manual-claim path reuses existing LSPS2 negotiation logic, only differing in the payment registration step where `create_inbound_payment_for_hash()` is used instead of `create_inbound_payment()`.

## Usage Example

See `examples/manual_claim_jit.rs` for a complete working example.

```rust
// Create manual-claim JIT invoice
let (payment_hash, invoice) = node
    .bolt11_payment()
    .receive_via_jit_channel_for_hash(
        25_000_000,  // 25,000 sats
        &description,
        3600,        // 1 hour expiry
        None         // No LSP fee limit
    )?;

// Wait for payment
loop {
    if let Some(Event::PaymentClaimable { payment_hash: hash, .. }) = node.next_event() {
        if hash == payment_hash {
            // Hold the HTLC for testing
            std::thread::sleep(Duration::from_secs(60));

            // Option 1: Claim with preimage
            // node.bolt11_payment().claim_for_hash(payment_hash)?;

            // Option 2: Fail the payment
            node.bolt11_payment().fail_for_hash(payment_hash)?;

            break;
        }
    }
    node.event_handled();
}
```

## Testing

### Running the Example

```bash
cd ldk-node

# Edit examples/manual_claim_jit.rs to set your LSP details:
# - lsp_pubkey_str
# - lsp_address
# - network
# - esplora_url

cargo run --example manual_claim_jit
```

The example will:

1. Start an LDK node
2. Create a manual-claim JIT invoice
3. Wait for payment
4. Hold the HTLC for 60 seconds
5. Fail the payment to release the HTLC

### Unit Tests

Unit tests should be added to verify:

- Payment hash is correctly registered as manual-claim
- `PaymentClaimable` events are emitted correctly
- No auto-claim occurs for manual-claim payments
- `claim_for_hash()` and `fail_for_hash()` work correctly

## Comparison with Existing Methods

| Method                                               | Auto-Claim | Returns                  | Use Case                        |
| ---------------------------------------------------- | ---------- | ------------------------ | ------------------------------- |
| `receive_via_jit_channel()`                          | ✅ Yes     | `Invoice`                | Normal JIT payments             |
| `receive_via_jit_channel_for_hash()`                 | ❌ No      | `(PaymentHash, Invoice)` | Testing, conditional acceptance |
| `receive_variable_amount_via_jit_channel()`          | ✅ Yes     | `Invoice`                | Zero-amount JIT payments        |
| `receive_variable_amount_via_jit_channel_for_hash()` | ❌ No      | `(PaymentHash, Invoice)` | Zero-amount with manual claim   |

## Security Considerations

1. **HTLC Timeout Risk**: Failing to claim or fail a payment before the CLTV deadline can result in force-closure of the channel.

2. **DoS Protection**: Holding HTLCs ties up liquidity. Applications should implement rate limiting and timeouts.

3. **Preimage Security**: Applications using `claim_for_hash()` must securely generate and store payment preimages.

## Future Improvements

1. **Preimage Generation Helper**: Add a utility function to generate cryptographically secure payment hash/preimage pairs.

2. **Automatic Timeout Handling**: Add configuration option to automatically fail held payments after a timeout.

3. **Payment Hold Statistics**: Track metrics on held payments for monitoring and debugging.

4. **Integration Tests**: Add full integration tests with mock LSP to verify end-to-end behavior.

## Related Methods

- `claim_for_hash(payment_hash: PaymentHash)` - Claim a manual-claim payment
- `fail_for_hash(payment_hash: PaymentHash)` - Fail a manual-claim payment
- `receive_for_hash()` - Manual-claim for regular (non-JIT) invoices
- `receive_variable_amount_for_hash()` - Manual-claim for zero-amount (non-JIT) invoices
