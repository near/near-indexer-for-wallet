use std::env;
use std::time::Duration;

use dotenv::dotenv;
#[macro_use]
extern crate diesel;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use futures::stream::StreamExt;
use itertools::Itertools;
use tokio::sync::mpsc;
use tokio::time;
use tokio_diesel::AsyncRunQueryDsl;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::db::enums::{ActionEnum, StatusEnum};
use crate::db::AccessKey;
use diesel::query_builder::AsQuery;

mod db;
mod schema;

const INTERVAL: Duration = Duration::from_millis(100);
const INDEXER_FOR_WALLET: &str = "indexer_for_wallet";

fn establish_connection() -> Pool<ConnectionManager<PgConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set in .env file"));
    let manager = ConnectionManager::<PgConnection>::new(&database_url);
    Pool::builder()
        .build(manager)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}

async fn handle_genesis_public_keys(near_config: near_indexer::NearConfig) {
    info!(
        target: INDEXER_FOR_WALLET,
        "Handling genesis public keys..."
    );
    let pool = establish_connection();
    info!(target: INDEXER_FOR_WALLET, "Connection to database established.");
    let genesis_height = near_config.genesis.config.genesis_height;
    let access_keys = near_config
        .genesis
        .records
        .as_ref()
        .iter()
        .filter_map(|record| {
            if let near_indexer::near_primitives::state_record::StateRecord::AccessKey {
                account_id,
                public_key,
                access_key,
            } = record
            {
                Some(AccessKey {
                    public_key: public_key.to_string(),
                    account_id: account_id.to_string(),
                    action: ActionEnum::Add,
                    status: StatusEnum::Success,
                    receipt_hash: "genesis".to_string(),
                    block_height: genesis_height.into(),
                    permission: (&access_key.permission).into(),
                })
            } else {
                None
            }
        }).take(20);

    info!(target: INDEXER_FOR_WALLET, "Iterator finished his work.");

    let chunk_size = 20;
    let total_access_key_chunks = access_keys.clone().count() / chunk_size;
    let slice = access_keys.chunks(chunk_size);

    // let insert_genesis_keys: futures::stream::FuturesUnordered<_> = slice
    //     .into_iter()
    //     .map(|keys| {
    //         diesel::insert_into(schema::access_keys::table)
    //             .values(keys.collect::<Vec<AccessKey>>())
    //             .on_conflict_do_nothing()
    //             .execute_async(&pool)
    //     })
    //     .collect();

    let insert_genesis_keys: futures::stream::FuturesUnordered<_> = slice
        .into_iter()
        .map(|keys| async {
            let collected_keys = keys.collect::<Vec<AccessKey>>();
            // let connection = pool.get().unwrap();
            loop {
                info!(target: INDEXER_FOR_WALLET, "Before insert...");
//                 match diesel::sql_query("
//         INSERT INTO public.access_keys (public_key, account_id, \"action\", status, receipt_hash, block_height, \"permission\")
// VALUES ('ed25519:74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 9571235, 'FULL_ACCESS'),
// ('ed25519:a74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957123, 'FULL_ACCESS'),
// ('ed25519:b74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 95712, 'FULL_ACCESS'),
// ('ed25519:c74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 95712313, 'FULL_ACCESS'),
// ('ed25519:d74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 9571235123, 'FULL_ACCESS'),
// ('ed25519:e74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957123534, 'FULL_ACCESS'),
// ('ed25519:f74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 95723, 'FULL_ACCESS'),
// ('ed25519:g74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 95712323434, 'FULL_ACCESS'),
// ('ed25519:h74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 95713455, 'FULL_ACCESS'),
// ('ed25519:i74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957768735, 'FULL_ACCESS'),
// ('ed25519:j74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957123567, 'FULL_ACCESS'),
// ('ed25519:k74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 9571232324, 'FULL_ACCESS'),
// ('ed25519:l74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957124567, 'FULL_ACCESS'),
// ('ed25519:m74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 9571238902, 'FULL_ACCESS'),
// ('ed25519:n74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957123232345, 'FULL_ACCESS'),
// ('ed25519:o74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 9123556, 'FULL_ACCESS'),
// ('ed25519:p74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957123546678, 'FULL_ACCESS'),
// ('ed25519:q74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 9571236578, 'FULL_ACCESS'),
// ('ed25519:r74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 9571232367876, 'FULL_ACCESS'),
// ('ed25519:s74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957123651235, 'FULL_ACCESS'),
// ('ed25519:t74zgLbkbjXBoyAspaj7XwMPeF9vMs8eV8MosSJygvuAZ', '0.testnet', 'ADD', 'SUCCESS', 'genesis', 957123512789, 'FULL_ACCESS')
// RETURNING \"access_keys\".\"public_key\", \"access_keys\".\"account_id\", \"access_keys\".\"action\", \"access_keys\".\"status\", \"access_keys\".\"receipt_hash\", \"access_keys\".\"block_height\", \"access_keys\".\"permission\"
//     ")
//                     .execute_async(&pool).await {
                info!(target:INDEXER_FOR_WALLET, "{}", diesel::debug_query(&diesel::insert_into(schema::access_keys::table)
                    .values(collected_keys.clone())
                    .on_conflict_do_nothing()
                    .as_query()).to_string());
                match diesel::insert_into(schema::access_keys::table)
                    .values(collected_keys.clone())
                    .on_conflict_do_nothing()
                    .as_query()
                    .execute_async(&pool).await {
//                 match diesel::insert_into(schema::access_keys::table)
//                     .values(collected_keys.clone())
//                     .on_conflict_do_nothing()
//                     .execute_async(&pool).await {
                    Ok(result) => {
                        info!(target: INDEXER_FOR_WALLET, "BREAK");
                        break result
                    },
                    Err(err) => {
                        info!(target: INDEXER_FOR_WALLET, "Trying to push genesis access keys failed with: {:?}. Retrying in {} seconds...", err, INTERVAL.as_secs_f32());
                        time::delay_for(INTERVAL).await;
                    }
                }
                info!(target: INDEXER_FOR_WALLET, "After insert...");
            }
        })
        .collect();

    let mut insert_genesis_keys = insert_genesis_keys.enumerate();

    // while let Some((index, result)) = insert_genesis_keys.next().await {
    //     match result {
    //         Ok(_) => { info!(target: INDEXER_FOR_WALLET, "Genesis public keys adding {}%", index * 100 / total_access_key_chunks); },
    //         Err(err) => { error!(target: INDEXER_FOR_WALLET, "Genesis public keys adding on {}% failed: \n {:?}", index * 100 / total_access_key_chunks, err); },
    //     };
    // }

    while let Some((index, _result)) = insert_genesis_keys.next().await {
        info!(target: INDEXER_FOR_WALLET, "Genesis public keys adding {}%", index * 100 / total_access_key_chunks);
    }

    info!(target: INDEXER_FOR_WALLET, "Genesis public keys handled.");
}

async fn update_receipt_status(
    receipt_ids: Vec<String>,
    status: StatusEnum,
    pool: &Pool<ConnectionManager<PgConnection>>,
) {
    loop {
        match diesel::update(
            schema::access_keys::table
                .filter(schema::access_keys::dsl::receipt_hash.eq_any(receipt_ids.clone())),
        )
        .set(schema::access_keys::dsl::status.eq(status))
        .execute_async(pool)
        .await
        {
            Ok(_) => break,
            Err(async_error) => {
                error!(
                    target: INDEXER_FOR_WALLET,
                    "Failed to update status, retrying in {} milliseconds... \n {:#?}",
                    INTERVAL.as_millis(),
                    async_error
                );
                time::delay_for(INTERVAL).await
            }
        }
    }
}

async fn listen_blocks(mut stream: mpsc::Receiver<near_indexer::BlockResponse>) {
    let pool = establish_connection();

    while let Some(block) = stream.recv().await {
        info!(
            target: INDEXER_FOR_WALLET,
            "Received block of height {:?}", block.block.header.height
        );

        // Handle outcomes
        let receipt_outcomes = &block
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                near_indexer::Outcome::Receipt(execution_outcome) => Some(execution_outcome),
                _ => None,
            })
            .collect::<Vec<&near_indexer::near_primitives::views::ExecutionOutcomeWithIdView>>();

        let failed_receipt_ids = receipt_outcomes
            .iter()
            .filter_map(|outcome| match &outcome.outcome.status {
                near_indexer::near_primitives::views::ExecutionStatusView::Failure(_) => {
                    Some(outcome.id.to_string())
                }
                _ => None,
            })
            .collect::<Vec<String>>();
        if failed_receipt_ids.len() > 0 {
            info!(
                target: INDEXER_FOR_WALLET,
                "Failed Receipt ExecutionOutcome received: {}",
                failed_receipt_ids.len()
            );
            update_receipt_status(failed_receipt_ids, StatusEnum::Failed, &pool).await;
        }

        let succeeded_receipt_ids = receipt_outcomes
            .iter()
            .filter_map(|outcome| match &outcome.outcome.status {
                near_indexer::near_primitives::views::ExecutionStatusView::SuccessReceiptId(_)
                | near_indexer::near_primitives::views::ExecutionStatusView::SuccessValue(_) => {
                    Some(outcome.id.to_string())
                }
                _ => None,
            })
            .collect::<Vec<String>>();
        if succeeded_receipt_ids.len() > 0 {
            info!(
                target: INDEXER_FOR_WALLET,
                "Succeeded Receipt ExecutionOutcome received: {}",
                succeeded_receipt_ids.len()
            );
            update_receipt_status(succeeded_receipt_ids, StatusEnum::Success, &pool).await;
        }

        // Handle receipts
        for chunk in &block.chunks {
            let receipts_to_add = chunk
                .receipts
                .iter()
                .filter_map(|receipt| match receipt.receipt {
                    near_indexer::near_primitives::views::ReceiptEnumView::Action { .. } => Some(
                        AccessKey::from_receipt_view(receipt, block.block.header.height),
                    ),
                    _ => None,
                })
                .flatten()
                .collect::<Vec<AccessKey>>();

            if receipts_to_add.len() > 0 {
                info!(
                    target: INDEXER_FOR_WALLET,
                    "{} receipts will be added",
                    receipts_to_add.len()
                );
                diesel::insert_into(schema::access_keys::table)
                    .values(receipts_to_add)
                    .execute_async(&pool)
                    .await
                    .unwrap();
            }
        }
    }
}

fn main() {
    // tokio_reactor=info,near=info,stats=info,telemetry=info,indexer_for_wallet=error,indexer_for_wallet=info
    let env_filter = EnvFilter::new("tokio_reactor=info,near=info,stats=info,telemetry=info,indexer_for_wallet=error,indexer_for_wallet=info");
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();

    info!(
        target: INDEXER_FOR_WALLET,
        "NEAR Indexer for Wallet started."
    );

    let home_dir: Option<String> = env::args().nth(1);
    let indexer = near_indexer::Indexer::new(home_dir.as_ref().map(AsRef::as_ref));
    let near_config = indexer.near_config().clone();
    let stream = indexer.streamer();
    actix::spawn(handle_genesis_public_keys(near_config));
    // actix::spawn(listen_blocks(stream));
    indexer.start();
}
