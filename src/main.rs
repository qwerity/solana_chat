mod utils;

use clap::Parser;
use solana_client::{
    rpc_client::{GetConfirmedSignaturesForAddress2Config, RpcClient},
    rpc_config::RpcSendTransactionConfig
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer, read_keypair_file},
    transaction::Transaction,
};
use solana_system_interface::{
    instruction,
    instruction::SystemInstruction,
};
use std::{
    str::FromStr,
    path::PathBuf,
    time::{Duration, Instant}
};
use utils::{
    current_timestamp,
    processed_signatures,
    transaction_default_config,
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    keypair: PathBuf,

    #[clap(long)]
    recipient: String,

    #[clap(long, default_value = "https://api.testnet.solana.com")]
    rpc_url: String,
}

const MEMO_TX_FEE_UI: u64 = 5_000;

async fn listen_for_memos(client: &RpcClient, sender: &Pubkey, recipient: &Pubkey) {
    let mut processed_signatures = processed_signatures(client, recipient);

    loop {
        let rpc_config = GetConfirmedSignaturesForAddress2Config {
            commitment: Some(CommitmentConfig::confirmed()),
            ..Default::default()
        };
        let tx_config = transaction_default_config();

        match client.get_signatures_for_address_with_config(recipient, rpc_config) {
            Ok(signatures) => {
                let mut new_signatures = Vec::new();
                for signature in signatures {
                    if processed_signatures.insert(signature.signature.clone()) {
                        new_signatures.push(signature);
                    }
                }

                for signature in new_signatures.into_iter().rev() {
                    if let Ok(transaction) = client.get_transaction_with_config(&signature.signature.parse().unwrap(), tx_config) {
                        if let Some(transaction_data) = transaction.transaction.transaction.decode() {
                            let program_ids = transaction_data.message.static_account_keys();
                            let instructions = transaction_data.message.instructions();
                            let mut has_transfer = false;
                            let mut memo = None;

                            if instructions.len() != 2 {
                                continue;
                            }

                            for ix in instructions {
                                let program_id = ix.program_id(program_ids);
                                let memo_id = Pubkey::new_from_array(spl_memo::ID.to_bytes());
                                if *program_id == memo_id {
                                    memo = Some(std::str::from_utf8(&ix.data).unwrap_or(""));
                                } else if *program_id == solana_system_program::id() {
                                    if let Ok(SystemInstruction::Transfer { lamports }) = bincode::deserialize::<SystemInstruction>(&ix.data) {
                                        if let (Some(source), Some(destination)) = (ix.accounts.first(), ix.accounts.get(1)) {
                                            let source_pubkey = transaction_data.message.static_account_keys()[*source as usize];
                                            let destination_pubkey = transaction_data.message.static_account_keys()[*destination as usize];
                                            if source_pubkey == *recipient && destination_pubkey == *sender {
                                                assert_eq!(lamports, 0);
                                                has_transfer = true;
                                            }
                                        }
                                    }
                                }
                            }

                            if has_transfer && memo.is_some() {
                                if let Some(memo_str) = memo {
                                    if !memo_str.is_empty() {
                                        println!("{} Received: {}", current_timestamp(), memo_str);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error getting signatures: {e}");
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn send_memo(client: &RpcClient, keypair: &Keypair, recipient: &Pubkey, memo: &str) {
    let transfer_instruction = instruction::transfer(&keypair.pubkey(), recipient, 0);
    let memo_instruction = Instruction {
        program_id: Pubkey::new_from_array(spl_memo::id().to_bytes()),
        accounts: vec![],
        data: memo.as_bytes().to_vec(),
    };
    let message = Message::new(&[transfer_instruction, memo_instruction], Some(&keypair.pubkey()));
    let blockhash = client.get_latest_blockhash().expect("Failed to get recent blockhash");
    let tx = Transaction::new(&[keypair], message, blockhash);

    let _now = Instant::now();
    match client.send_transaction_with_config(
        &tx,
        RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
            ..Default::default()
        },
    ) {
        Ok(_signature) => {
            // println!("Confirmation time: {:.2?}, https://solscan.io/tx/{}?cluster=testnet", now.elapsed(), _signature);
            println!("{} Sent memo: {}", current_timestamp(), memo);
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let sender = read_keypair_file(&args.keypair).expect("Unable to read sender keypair file");
    let sender_pubkey = sender.pubkey();
    let recipient = Pubkey::from_str(&args.recipient).expect("Invalid recipient public key");
    let client = RpcClient::new_with_commitment(&args.rpc_url, CommitmentConfig::confirmed());

    let balance = client.get_balance(&sender_pubkey).expect("Failed to get balance");
    if balance >= MEMO_TX_FEE_UI {
        let balance_ui = balance as f64 / solana_sdk::native_token::LAMPORTS_PER_SOL as f64;
        println!("You are able to send {} messages, balance: {balance_ui}", balance / MEMO_TX_FEE_UI);
    } else {
        eprintln!("Not enough balance to send memo messages");
        std::process::exit(1);
    }

    tokio::spawn(async move {
        let client = RpcClient::new_with_commitment(&args.rpc_url, CommitmentConfig::confirmed());
        listen_for_memos(&client, &sender_pubkey, &recipient).await;
    });

    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect("Failed to read message");

        let memo = input.trim();
        if memo.is_empty() {
            continue;
        }

        send_memo(&client, &sender, &recipient, memo).await;
    }
}
