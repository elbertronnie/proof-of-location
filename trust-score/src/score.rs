use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use subxt::config::substrate::AccountId32;
use subxt::{OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::dev;

use substrate::runtime_types::pallet_template::pallet::LocationData;

// This creates a complete, type-safe API for interacting with the runtime.
#[subxt::subxt(runtime_metadata_path = "../metadata.scale")]
pub mod substrate {}

// Create a new median scheme where 1/4 of the highest values are discarded
pub fn trimmed_median_error(values: &mut [i16]) -> i16 {
    if values.len() < 4 {
        return i16::MAX;
    }

    values.iter_mut().for_each(|x| *x = x.abs());
    values.sort_unstable();

    let len = values.len();
    let trim_end = (len * 3 / 4) as usize;
    let trimmed = &values[..trim_end];

    if trim_end % 2 == 1 {
        trimmed[trim_end / 2]
    } else {
        let mid_upper = trimmed[trim_end / 2];
        let mid_lower = trimmed[trim_end / 2 - 1];
        (mid_upper + mid_lower) / 2
    }
}

pub fn get_account_names() -> HashMap<[u8; 32], &'static str> {
    let mut names = HashMap::new();

    names.insert(dev::alice().public_key().0, "Alice");
    names.insert(dev::bob().public_key().0, "Bob");
    names.insert(dev::charlie().public_key().0, "Charlie");
    names.insert(dev::dave().public_key().0, "Dave");
    names.insert(dev::eve().public_key().0, "Eve");
    names.insert(dev::ferdie().public_key().0, "Ferdie");

    names
}

pub async fn fetch_all_location_data(
    api: &OnlineClient<SubstrateConfig>,
) -> Result<HashMap<[u8; 32], LocationData>, Box<dyn std::error::Error>> {
    // Use the generated API to access the storage
    let query = substrate::storage().template().account_data_iter();

    // Fetch all account data
    let mut account_data = api.storage().at_latest().await?.iter(query).await?;

    // Store results inside a vector
    let mut results = HashMap::new();

    // Collect all results
    while let Some(Ok(data)) = account_data.next().await {
        // Skip first 32 bytes (pallet + storage hashes) and 16 bytes (blake2_128 hash)
        // Take the last 32 bytes as the account ID
        let len = data.key_bytes.len();
        let account_id: [u8; 32] = data.key_bytes[len - 32..].try_into().unwrap();
        results.insert(account_id, data.value);
    }

    Ok(results)
}

pub async fn fetch_rssi(
    api: &OnlineClient<SubstrateConfig>,
    block_number: u32,
    account: AccountId32,
    reporter: AccountId32,
) -> Result<Option<i16>, Box<dyn std::error::Error>> {
    // Use the generated API to access the storage
    let query = substrate::storage()
        .template()
        .rssi_data(block_number, account, reporter);

    // Fetch the RSSI value
    let rssi_value = api.storage().at_latest().await?.fetch(&query).await?;

    Ok(rssi_value)
}

pub async fn fetch_all_rssi(
    api: &OnlineClient<SubstrateConfig>,
    block_number: u32,
    account: AccountId32,
) -> Result<HashMap<[u8; 32], i16>, Box<dyn std::error::Error>> {
    // Use the generated API to access the storage
    let query = substrate::storage()
        .template()
        .rssi_data_iter2(block_number, &account);

    // Fetch all account data
    let mut rssi_data = api.storage().at_latest().await?.iter(query).await?;

    // Store results inside a HashMap
    let mut results = HashMap::new();

    // Collect all results
    while let Some(Ok(data)) = rssi_data.next().await {
        let len = data.key_bytes.len();
        let account_id: [u8; 32] = data.key_bytes[len - 32..].try_into().unwrap();
        results.insert(account_id, data.value);
    }

    Ok(results)
}

const PATH_LOSS_EXPONENT: f64 = 3.0;

pub fn estimate_rssi(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> i16 {
    use haversine_redux::Location;
    let a = Location::new(a_lat, a_lon);
    let b = Location::new(b_lat, b_lon);
    let dist = a.kilometers_to(&b) * 1000.0; // convert kilometers to meters
    let rssi = if dist != 0.0 {
        -60.0 - PATH_LOSS_EXPONENT * 10.0 * dist.log10()
    } else {
        0.0
    };
    rssi as i16
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

        let all_accounts = fetch_all_location_data(&api).await?;

        let mut errors = HashMap::new();
        for (account, location_data) in &all_accounts {
            let rssi_data =
                fetch_all_rssi(&api, block.number(), AccountId32::from(*account)).await?;

            for (reporter, rssi) in &rssi_data {
                if let Some(_) = fetch_rssi(
                    &api,
                    block.number(),
                    AccountId32::from(*account),
                    AccountId32::from(*reporter),
                )
                .await?
                {
                    let reporter_location_data = all_accounts.get(reporter).unwrap();
                    let estimated_rssi = estimate_rssi(
                        location_data.latitude as f64 / 1_000_000.0,
                        location_data.longitude as f64 / 1_000_000.0,
                        reporter_location_data.latitude as f64 / 1_000_000.0,
                        reporter_location_data.longitude as f64 / 1_000_000.0,
                    );

                    let error = *rssi - estimated_rssi;
                    errors.entry(*account).or_insert_with(Vec::new).push(error);
                }
            }
        }

        let mut new_error_data: Vec<ErrorData> = errors
            .into_iter()
            .map(|(k, mut v)| ErrorData {
                account_name: account_name.get(&k).unwrap_or(&"Unknown").to_string(),
                error_value: trimmed_median_error(&mut v),
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
