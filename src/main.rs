use std::env;
use std::time::Duration;

use dotenv::dotenv;
#[macro_use]
extern crate diesel;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, PgConnection, QueryDsl};
use futures::stream::StreamExt;
use itertools::Itertools;
use tokio::sync::mpsc;
use tokio::time;
use tokio_diesel::AsyncRunQueryDsl;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::db::enums::{ActionEnum, StatusEnum};
use crate::db::AccessKey;

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
    let pool = establish_connection();
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
        });

    let chunk_size = 5000;
    let total_access_key_chunks = access_keys.clone().count() / chunk_size;
    let total_access_key_chunks = if total_access_key_chunks > 0 {
        total_access_key_chunks
    } else {
        1
    };
    let slice = access_keys.chunks(chunk_size);

    let insert_genesis_keys: futures::stream::FuturesUnordered<_> = slice
        .into_iter()
        .map(|keys| async {
            let collected_keys = keys.collect::<Vec<AccessKey>>();
            loop {
                match diesel::insert_into(schema::access_keys::table)
                    .values(collected_keys.clone())
                    .on_conflict_do_nothing()
                    .execute_async(&pool).await {
                        Ok(result) => break result,
                        Err(err) => {
                            info!(target: INDEXER_FOR_WALLET, "Trying to push genesis access keys failed with: {:?}. Retrying in {} seconds...", err, INTERVAL.as_secs_f32());
                            time::delay_for(INTERVAL).await;
                        }
                    }
                }
            })
        .collect();

    let mut insert_genesis_keys = insert_genesis_keys.enumerate();

    while let Some((index, _result)) = insert_genesis_keys.next().await {
        info!(
            target: INDEXER_FOR_WALLET,
            "Genesis public keys adding {}%",
            index * 100 / total_access_key_chunks
        );
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
                    target: "indexer_for_wallet", "Failed to update status, retrying in {} milliseconds... \n {:#?}",
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
        eprintln!("Block height {:?}", block.block.header.height);

        // Handle receipts
        for chunk in &block.chunks {
            diesel::insert_into(schema::access_keys::table)
                .values(
                    chunk
                        .receipts
                        .iter()
                        .filter_map(|receipt| match receipt.receipt {
                            near_indexer::near_primitives::views::ReceiptEnumView::Action {
                                ..
                            } => Some(AccessKey::from_receipt_view(
                                receipt,
                                block.block.header.height,
                            )),
                            _ => None,
                        })
                        .flatten()
                        .collect::<Vec<AccessKey>>(),
                )
                .execute_async(&pool)
                .await
                .unwrap();
        }

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
        update_receipt_status(failed_receipt_ids, StatusEnum::Failed, &pool).await;

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
        update_receipt_status(succeeded_receipt_ids, StatusEnum::Success, &pool).await;
    }
}

fn main() {
    let env_filter = EnvFilter::new("tokio_reactor=info,near=info,near=error,stats=info,telemetry=info,indexer_for_wallet=error,indexer_for_wallet=info");
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
    actix::spawn(listen_blocks(stream));
    indexer.start();
}
