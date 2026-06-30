#![allow(unused)]
use bitcoin::hex::DisplayHex;
use bitcoincore_rpc::bitcoin::Amount;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;
// Node access params
const RPC_URL: &str = "http://127.0.0.1:18443"; // Default regtest RPC port
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";
// You can use calls not provided in RPC lib API using the generic `call` function.
// An example of using the `send` RPC call, which doesn't have exposed API.
// You can also use serde_json `Deserialize` derivation to capture the returned json result.
fn send(rpc: &Client, addr: &str) -> bitcoincore_rpc::Result<String> {
    let args = [
        json!([{addr : 100 }]), // recipient address
        json!(null),            // conf target
        json!(null),            // estimate mode
        json!(null),            // fee rate in sats/vb
        json!(null),            // Empty option object
    ];
    #[derive(Deserialize)]
    struct SendResult {
        complete: bool,
        txid: String,
    }
    let send_result = rpc.call::<SendResult>("send", &args)?;
    assert!(send_result.complete);
    Ok(send_result.txid)
}
fn main() -> bitcoincore_rpc::Result<()> {
    // Connect to Bitcoin Core RPC
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;
    // Get blockchain info
    let blockchain_info = rpc.get_blockchain_info()?;
    println!("Blockchain Info: {:?}", blockchain_info);

    // Create/Load the wallets, named 'Miner' and 'Trader'. Have logic to optionally create/load them if they do not exist or not loaded already.
    let miner_wallet = "Miner";
    let trader_wallet = "Trader";

    for wallet in [miner_wallet, trader_wallet] {
        let loaded = rpc.list_wallets()?;
        if loaded.iter().any(|w| w == wallet) {
            continue;
        }
        let on_disk = rpc.list_wallet_dir()?;
        if on_disk.iter().any(|w| w == wallet) {
            rpc.load_wallet(wallet)?;
        } else {
            rpc.create_wallet(wallet, None, None, None, None)?;
        }
    }

    // Wallet-scoped clients — bitcoincore-rpc requires a client pointed at
    // /wallet/<name> to operate on a specific wallet; there's no call
    // wallet parameter.
    let miner_rpc = Client::new(
        &format!("{}/wallet/{}", RPC_URL, miner_wallet),
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;
    let trader_rpc = Client::new(
        &format!("{}/wallet/{}", RPC_URL, trader_wallet),
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Generate a new address from Miner wallet, labeled "Mining Reward"
    let miner_address = miner_rpc
        .get_new_address(Some("Mining Reward"), None)?
        .assume_checked();

    // Generate spendable balances in the Miner wallet. How many blocks needs to be mined?
    // Coinbase outputs require 100 confirmations (COINBASE_MATURITY) before
    // they're spendable. Mining a block credits the reward immediately, but
    // it stays "immature" until 100 more blocks are mined on top of it. So
    // the wallet balance stays at 0 until block 101, when block 1's reward
    // finally matures and becomes spendable.
    let mut blocks_mined: u64 = 0;
    let mut miner_balance = miner_rpc.get_balance(None, None)?;

    while miner_balance == Amount::from_sat(0) {
        miner_rpc.generate_to_address(1, &miner_address)?;
        blocks_mined += 1;
        miner_balance = miner_rpc.get_balance(None, None)?;
    }

    println!("Mined {} blocks before positive balance.", blocks_mined);
    println!("Miner wallet balance: {}", miner_balance);

    // Load Trader wallet and generate a new address
    let trader_address = trader_rpc
        .get_new_address(Some("Received"), None)?
        .assume_checked();

    // Send 20 BTC from Miner to Trader
    let send_amount = Amount::from_btc(20.0)?;
    let txid = miner_rpc.send_to_address(
        &trader_address,
        send_amount,
        None,
        None,
        None,
        None,
        None,
        None,
    )?;

    // Check transaction in mempool
    let mempool_entry = rpc.get_mempool_entry(&txid)?;
    println!("Mempool entry: {:?}", mempool_entry);

    // Decode the tx to identify the miner's input (prevout) and outputs
    let decoded = miner_rpc.get_raw_transaction_info(&txid, None)?;

    let vin = &decoded.vin[0];
    let prev_txid = vin.txid.expect("missing prevout txid");
    let prev_vout = vin.vout.expect("missing prevout vout");
    let prev_tx_info = miner_rpc.get_raw_transaction_info(&prev_txid, None)?;
    let prev_out = &prev_tx_info.vout[prev_vout as usize];
    let miner_input_address = prev_out
        .script_pub_key
        .address
        .clone()
        .expect("no address on prevout")
        .assume_checked();
    let miner_input_amount = prev_out.value;

    let mut trader_output_amount = Amount::from_sat(0);
    let mut change_address = None;
    let mut change_amount = Amount::from_sat(0);

    for out in &decoded.vout {
        let addr = out
            .script_pub_key
            .address
            .clone()
            .map(|a| a.assume_checked());
        match addr {
            Some(a) if a == trader_address => {
                trader_output_amount = out.value;
            }
            Some(a) => {
                change_address = Some(a);
                change_amount = out.value;
            }
            None => {}
        }
    }
    let change_address = change_address.expect("change address not found");

    // Fee, as reported by the wallet (negative SignedAmount)
    let wallet_tx = miner_rpc.get_transaction(&txid, None)?;
    let fee_btc = wallet_tx.fee.unwrap().to_btc();

    // Mine 1 block to confirm the transaction
    let confirm_address = miner_rpc.get_new_address(None, None)?.assume_checked();
    let confirm_hashes = miner_rpc.generate_to_address(1, &confirm_address)?;
    let confirm_block_hash = confirm_hashes[0];

    // Extract all required transaction details
    let final_tx = miner_rpc.get_transaction(&txid, None)?;
    let confirm_height = final_tx.info.blockheight.unwrap();
    println!("Final transaction: {:?}", final_tx);

    // Write the data to ../out.txt in the specified format given in readme.md
    let mut file = File::create("../out.txt")?;
    writeln!(file, "{}", txid)?;
    writeln!(file, "{}", miner_input_address)?;
    writeln!(file, "{}", miner_input_amount.to_btc())?;
    writeln!(file, "{}", trader_address)?;
    writeln!(file, "{}", trader_output_amount.to_btc())?;
    writeln!(file, "{}", change_address)?;
    writeln!(file, "{}", change_amount.to_btc())?;
    writeln!(file, "{}", fee_btc)?;
    writeln!(file, "{}", confirm_height)?;
    writeln!(file, "{}", confirm_block_hash)?;

    Ok(())
}
