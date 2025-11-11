use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use subxt::config::substrate::AccountId32;
use subxt::{OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::dev;

// This creates a complete, type-safe API for interacting with the runtime.
#[subxt::subxt(runtime_metadata_path = "../metadata.scale")]
pub mod substrate {}

fn get_account_names() -> HashMap<[u8; 32], &'static str> {
    let mut names = HashMap::new();

    names.insert(dev::alice().public_key().0, "Alice");
    names.insert(dev::bob().public_key().0, "Bob");
    names.insert(dev::charlie().public_key().0, "Charlie");
    names.insert(dev::dave().public_key().0, "Dave");
    names.insert(dev::eve().public_key().0, "Eve");
    names.insert(dev::ferdie().public_key().0, "Ferdie");

    names
}

#[derive(Clone)]
pub struct ErrorData {
    pub account_name: String,
    pub error_value: i16,
}

pub async fn blockchain_task(
    error_data: Arc<Mutex<Vec<ErrorData>>>,
    block_number: Arc<Mutex<u32>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get RPC URL from environment variable or use default
    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".into());

    // Connect to the node
    println!("Connecting to node at {}...", rpc_url);
    let api = OnlineClient::<SubstrateConfig>::from_url(&rpc_url).await?;

    println!("Connected successfully!\n");

    let account_name = get_account_names();

    let mut blocks_sub = api.blocks().subscribe_finalized().await?;
    while let Some(Ok(block)) = blocks_sub.next().await {
        if block.number() < 3 {
            continue;
        }

        println!("New finalized block: {}", block.number());
        *block_number.lock().unwrap() = block.number();

        let rpc_call = substrate::apis()
            .trust_score_api()
            .calculate_trust_scores(block.number());

        // Call the trust score RPC using the generated API
        let scores: Vec<(AccountId32, i16)> =
            api.runtime_api().at_latest().await?.call(rpc_call).await?;

        // Convert to ErrorData format
        let mut new_error_data: Vec<ErrorData> = scores
            .into_iter()
            .map(|(account_id, error_value)| ErrorData {
                account_name: account_name
                    .get(&account_id.0)
                    .unwrap_or(&"Unknown")
                    .to_string(),
                error_value,
            })
            .collect();

        new_error_data.sort_by_key(|x| x.account_name.clone());

        for ErrorData {
            account_name,
            error_value,
        } in &new_error_data
        {
            println!("{}: {}", account_name, error_value);
        }

        // Update the shared error data
        *error_data.lock().unwrap() = new_error_data;
    }

    Ok(())
}
