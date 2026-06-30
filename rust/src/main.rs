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
        json!([{addr : 20 }]), // recipient address
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

    // ================================
    // CREATE OR LOAD WALLETS
    // ================================

    let miner_wallet = "Miner";
    let trader_wallet = "Trader";

    // Try load Miner wallet, if it fails create it
    let miner = match rpc.load_wallet(miner_wallet) {
        Ok(w) => w,
        Err(_) => rpc.create_wallet(miner_wallet, None, None, None, None)?,
    };

    // Try load Trader wallet, if it fails create it
    let trader = match rpc.load_wallet(trader_wallet) {
        Ok(w) => w,
        Err(_) => rpc.create_wallet(trader_wallet, None, None, None, None)?,
    };


    // Generate spendable balances in the Miner wallet. How many blocks needs to be mined?

    // ================================
    // GENERATE COINS FOR MINER
    // ================================

    // Get a new address from miner wallet
    let miner_address = miner.get_new_address(None, None)?;

    // Mine 101 blocks. 100 are needed to mature coinbase in regtest
    let _blocks = rpc.generate_to_address(101, &miner_address)?;



    // Load Trader wallet and generate a new address

    // ================================
    // TRADER WALLET ADDRESS
    // ================================

    let trader_address = trader.get_new_address(None, None)?;


    // Send 20 BTC from Miner to Trader

    // ================================
    // SEND BTC (MINER → TRADER)
    // ================================

    let txid = send(&miner, &trader_address.to_string())?;
    println!("Transaction sent: {}", txid);



    // Check transaction in mempool

    // ================================
    // CHECK MEMPOOL
    // ================================

    let mempool = rpc.get_raw_mempool()?;
    println!("Mempool: {:?}", mempool);



    // Mine 1 block to confirm the transaction

    // Extract all required transaction details

    // Write the data to ../out.txt in the specified format given in readme.md

    Ok(())
}
