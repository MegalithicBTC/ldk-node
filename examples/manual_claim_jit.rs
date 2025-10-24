// Example demonstrating manual-claim JIT channel behavior
// This shows how to hold HTLCs by not claiming payments immediately

use ldk_node::bitcoin::{secp256k1::PublicKey, Network};
use ldk_node::config::{AnchorChannelsConfig, Config, EsploraSyncConfig, BackgroundSyncConfig};
use ldk_node::lightning_invoice::{Bolt11InvoiceDescription, Description};
use ldk_node::logger::LogLevel;
use ldk_node::Builder;
use ldk_node::Event;
use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== Manual-Claim JIT Channel Test (Hold HTLC Indefinitely) ===\n");

    // Load environment variables from .env file if present
    let _ = dotenvy::dotenv();

    // ── Configuration from environment ─────────────────────────────────────
    let lsp_pubkey_str = env::var("LSP_PUBKEY")
        .expect("LSP_PUBKEY must be set in .env or environment");
    let lsp_address = env::var("LSP_ADDRESS")
        .expect("LSP_ADDRESS must be set in .env or environment");
    
    let network_str = env::var("NETWORK").unwrap_or_else(|_| "bitcoin".to_string());
    let network = match network_str.to_lowercase().as_str() {
        "bitcoin" => Network::Bitcoin,
        "testnet" => Network::Testnet,
        "regtest" => Network::Regtest,
        "signet" => Network::Signet,
        _ => {
            eprintln!("Warning: Unknown network '{}', defaulting to Bitcoin", network_str);
            Network::Bitcoin
        }
    };
    
    let esplora_url = env::var("ESPLORA_API_URL")
        .unwrap_or_else(|_| "https://blockstream.info/api".to_string());
    
    let log_level_str = env::var("LOG_LEVEL").unwrap_or_else(|_| "Debug".to_string());
    let log_level = match log_level_str.to_lowercase().as_str() {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "error" => LogLevel::Error,
        _ => {
            eprintln!("Warning: Unknown log level '{}', defaulting to Debug", log_level_str);
            LogLevel::Debug
        }
    };

    // Parse LSP pubkey
    let lsp_pubkey = PublicKey::from_str(&lsp_pubkey_str)
        .expect("Invalid LSP_PUBKEY format");

    // ── Setup Node ─────────────────────────────────────────────────────────
    let storage_dir = "tmp_manual_claim".to_string();
    let log_path = format!("{}/manual_claim_jit.log", storage_dir);

    let mut cfg = Config::default();
    cfg.network = network;
    
    // CRITICAL: Set payment claim policy to Manual to prevent auto-claiming
    cfg.payment_claim_policy = ldk_node::config::PaymentClaimPolicy::Manual;

    // Configure anchor channels with LSP as trusted peer (no reserve)
    let mut anchor_cfg = AnchorChannelsConfig::default();
    anchor_cfg.trusted_peers_no_reserve.push(lsp_pubkey);
    cfg.anchor_channels_config = Some(anchor_cfg);

    let mut builder = Builder::from_config(cfg);

    // Configure sync intervals
    let mut sync_config = EsploraSyncConfig::default();
    sync_config.background_sync_config = Some(BackgroundSyncConfig {
        onchain_wallet_sync_interval_secs: 120,
        lightning_wallet_sync_interval_secs: 60,
        fee_rate_cache_update_interval_secs: 300,
    });

    builder
        .set_storage_dir_path(storage_dir.clone())
        .set_filesystem_logger(Some(log_path.clone()), Some(log_level))
        .set_chain_source_esplora(esplora_url, Some(sync_config))
        .set_liquidity_source_lsps2(
            lsp_pubkey,
            lsp_address.parse().expect("Invalid LSP_ADDRESS format"),
            None,
        );

    let node = Arc::new(builder.build().expect("Failed to build node"));

    if let Err(e) = node.start() {
        eprintln!("WARNING: Node startup issue: {}", e);
        eprintln!("Continuing anyway - node may still work for some operations.");
    }

    println!("Node started successfully!");
    println!("Node ID: {}", node.node_id());
    println!("Logs: {}\n", log_path);

    // ── Create Manual-Claim JIT Invoice ────────────────────────────────────
    let amount_msat = 25_000_000; // 25,000 sats
    
    // Parse invoice expiry from environment
    let expiry_secs = env::var("INVOICE_EXPIRY_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600); // Default: 1 hour
    
    // Parse min_final_cltv_expiry_delta from environment (THIS controls HTLC timeout!)
    // Default based on network if not specified
    let default_cltv_delta = match network {
        Network::Regtest => 10,   // Fast testing on regtest
        Network::Testnet | Network::Signet => 18,  // ~3 hours on testnet
        Network::Bitcoin => 80,   // ~13 hours on mainnet (safe default)
        _ => 80,
    };
    
    let min_final_cltv_expiry_delta: u16 = env::var("MIN_FINAL_CLTV_EXPIRY_DELTA")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_cltv_delta);
    
    println!("Creating manual-claim JIT invoice:");
    println!("  Amount: {} msats ({} sats)", amount_msat, amount_msat / 1000);
    println!("  Invoice Expiry: {} seconds (~{} minutes)", expiry_secs, expiry_secs / 60);
    println!("  HTLC CLTV Delta: {} blocks", min_final_cltv_expiry_delta);
    println!("  Network: {:?}", network);
    
    let estimated_timeout = match network {
        Network::Regtest => format!("{} blocks (instant with manual mining)", min_final_cltv_expiry_delta),
        Network::Bitcoin | Network::Testnet | Network::Signet => {
            let minutes = min_final_cltv_expiry_delta as u64 * 10; // ~10 min per block
            format!("{} blocks (~{:.1} hours)", min_final_cltv_expiry_delta, minutes as f64 / 60.0)
        },
        _ => format!("{} blocks", min_final_cltv_expiry_delta),
    };
    println!("  Estimated HTLC Timeout: {}", estimated_timeout);
    println!();
    
    let desc = Bolt11InvoiceDescription::Direct(
        Description::new("manual-claim-test-htlc-hold".into()).unwrap(),
    );

    println!("Creating manual-claim JIT invoice for {} msats...", amount_msat);

    let (payment_hash, invoice) = node
        .bolt11_payment()
        .receive_via_jit_channel_for_hash(amount_msat, &desc, expiry_secs, None, Some(min_final_cltv_expiry_delta))
        .expect("Failed to create manual-claim JIT invoice");

    println!("\n=== MANUAL-CLAIM JIT INVOICE ===");
    println!("Payment Hash: {:?}", payment_hash);
    println!("Invoice:\n{}\n", invoice);
    println!("================================\n");

    println!("INSTRUCTIONS:");
    println!("1. Pay this invoice from another node");
    println!("2. The LSP will open a JIT channel");
    println!("3. When payment arrives, you'll see a PaymentClaimable event");
    println!("4. This example will HOLD the HTLC (not claim it immediately)");
    println!("5. You can then manually claim or fail the payment\n");

    // ── Setup Ctrl-C handler ───────────────────────────────────────────────
    let node_clone = Arc::clone(&node);
    let payment_hash_clone = payment_hash;
    std::thread::spawn(move || {
        let _ = std::io::stdin().read_line(&mut String::new());
        println!("\n\nShutting down node...");
        println!("Failing any held payment before shutdown...");
        // Try to fail pending payment before shutdown
        let _ = node_clone.bolt11_payment().fail_for_hash(payment_hash_clone);
        let _ = node_clone.stop();
        std::process::exit(0);
    });

    // ── Event Loop: Watch for PaymentClaimable ────────────────────────────
    println!("Monitoring for payment events...");
    println!("Press Enter to shutdown\n");

    loop {
        if let Some(event) = node.next_event() {
            match event {
                Event::PaymentClaimable {
                    payment_id,
                    payment_hash: event_hash,
                    claimable_amount_msat,
                    claim_deadline,
                    ..
                } => {
                    if event_hash == payment_hash {
                        println!("\n🎯 PAYMENT CLAIMABLE EVENT RECEIVED!");
                        println!("   Payment ID: {:?}", payment_id);
                        println!("   Payment Hash: {:?}", event_hash);
                        println!("   Amount: {} msats ({} sats)", claimable_amount_msat, claimable_amount_msat / 1000);
                        if let Some(deadline) = claim_deadline {
                            println!("   Claim Deadline: {} blocks", deadline);
                        }
                        println!("\n⏸️  HOLDING HTLC INDEFINITELY (not claiming or failing)");
                        println!("   This demonstrates holding payment in limbo");
                        println!("   The HTLC will remain pending until:");
                        println!("   - You press Enter to shutdown (will auto-fail)");
                        println!("   - The CLTV deadline expires (LSP may force-close)");
                        println!("   - You manually claim/fail via code modification\n");
                        
                        // DO NOT claim or fail - just hold indefinitely
                        // To claim:  node.bolt11_payment().claim_for_hash(payment_hash)?;
                        // To fail:   node.bolt11_payment().fail_for_hash(payment_hash)?;
                    }
                }
                Event::ChannelPending {
                    channel_id,
                    counterparty_node_id,
                    ..
                } => {
                    println!("📢 Channel pending: {:?} with {}", 
                        channel_id, 
                        counterparty_node_id);
                }
                Event::ChannelReady {
                    channel_id,
                    counterparty_node_id,
                    ..
                } => {
                    println!("✅ Channel ready: {:?} with {:?}", 
                        channel_id, 
                        counterparty_node_id);
                }
                _ => {
                    // Ignore other events for this test
                }
            }

            let _ = node.event_handled();
        }
        
        // Brief sleep to avoid busy loop
        thread::sleep(Duration::from_millis(100));
    }
}
