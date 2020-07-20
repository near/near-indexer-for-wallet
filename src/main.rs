use std::env;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time;

use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
#[macro_use]
extern crate diesel;
extern crate dotenv;
use dotenv::dotenv;
use tokio_diesel::*;

use near_indexer;

mod schema;
mod db;
use db::{AccessKey};
use db::enums::{ActionEnum, StatusEnum, PermissionEnum};

const INTERVAL: Duration = Duration::from_millis(100);
const TIMES_TO_RETRY: u8 = 10;

fn establish_connection() -> Pool<ConnectionManager<PgConnection>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env file");
    let manager = ConnectionManager::<PgConnection>::new(&database_url);
    Pool::builder().build(manager).expect(&format!("Error connecting to {}", database_url))
}

async fn handle_genesis_public_keys(near_config: near_indexer::NearConfig) {
    let pool = establish_connection();
    let genesis_height = near_config.genesis.config.genesis_height.clone();
    let access_keys = near_config.genesis.records.as_ref()
        .iter()
        .filter(|record| match record {
        near_indexer::near_primitives::state_record::StateRecord::AccessKey { .. } => true,
        _ => false,
    })
        .filter_map(|record| if let near_indexer::near_primitives::state_record::StateRecord::AccessKey { account_id, public_key, access_key} = record {
        Some(AccessKey {
            public_key: public_key.to_string(),
            account_id: account_id.to_string(),
            action: ActionEnum::Add,
            status: StatusEnum::Success,
            receipt_hash: "genesis".to_string(),
            block_height: genesis_height.into(),
            permission: match access_key.permission {
                near_indexer::near_primitives::account::AccessKeyPermission::FullAccess => PermissionEnum::FullAccess,
                near_indexer::near_primitives::account::AccessKeyPermission::FunctionCall(_) => PermissionEnum::FunctionCall,
            }
        })
    } else { None }).collect::<Vec<AccessKey>>();

    diesel::insert_into(schema::access_keys::table)
        .values(access_keys)
        .on_conflict_do_nothing()
        .execute_async(&pool)
        .await
        .unwrap();
}

async fn listen_blocks(mut stream: mpsc::Receiver<near_indexer::BlockResponse>) {
    let pool = establish_connection();

    while let Some(block) = stream.recv().await {
        eprintln!("Block height {:?}", block.block.header.height);

        // Handle outcomes
        let receipt_outcomes = &block
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                near_indexer::Outcome::Receipt(execution_outcome) => Some(execution_outcome),
                _ => None,
            })
            .collect::<Vec<&near_indexer::near_primitives::views::ExecutionOutcomeWithIdView>>();

        for _ in 1..=TIMES_TO_RETRY {
            match diesel::update(
                schema::access_keys::table.filter(
                    schema::access_keys::dsl::receipt_hash.eq_any(
                        receipt_outcomes
                            .iter()
                            .filter_map(|outcome| match &outcome.outcome.status {
                                near_indexer::near_primitives::views::ExecutionStatusView::Failure(
                                    _,
                                ) => Some(outcome.id.to_string()),
                                _ => None,
                            })
                            .collect::<Vec<String>>(),
                    ),
                ),
            )
            .set(schema::access_keys::dsl::status.eq(StatusEnum::Failed))
            .execute_async(&pool)
            .await {
                Ok(_) => break,
                Err(_) => time::delay_for(INTERVAL).await,
            }
        }

        for _ in 1..=TIMES_TO_RETRY {
            match diesel::update(
                schema::access_keys::table.filter(
                    schema::access_keys::dsl::receipt_hash.eq_any(
                        receipt_outcomes
                            .iter()
                            .filter_map(|outcome| match &outcome.outcome.status {
                                near_indexer::near_primitives::views::ExecutionStatusView::SuccessReceiptId(
                                    _,
                                ) | near_indexer::near_primitives::views::ExecutionStatusView::SuccessValue(_) => Some(outcome.id.to_string()),
                                _ => None,
                            })
                            .collect::<Vec<String>>(),
                    ),
                ),
            )
                .set(schema::access_keys::dsl::status.eq(StatusEnum::Success))
                .execute_async(&pool)
                .await {
                Ok(_) => break,
                Err(_) => time::delay_for(INTERVAL).await,
            }
        }

        // Handle receipts
        for chunk in &block.chunks {
            diesel::insert_into(schema::access_keys::table)
                .values(
                    chunk
                        .receipts
                        .iter()
                        .filter_map(|receipt| match receipt.receipt {
                            near_indexer::near_primitives::views::ReceiptEnumView::Action { .. } => Some(AccessKey::from_receipt_view(receipt, block.block.header.height)),
                            _ => None,
                        })
                        .flatten()
                        .collect::<Vec<AccessKey>>(),
                )
                .execute_async(&pool)
                .await
                .unwrap();
        }
    }
}

fn main() {
    let home_dir: Option<String> = env::args().nth(1);

    let indexer = near_indexer::Indexer::new(home_dir.as_ref().map(AsRef::as_ref));
    let near_config = indexer.near_config().clone();
    let stream = indexer.streamer();
    actix::spawn(handle_genesis_public_keys(near_config));
    actix::spawn(listen_blocks(stream));
    indexer.start();
}
