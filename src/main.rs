use std::collections::HashMap;
use std::time::Duration;

use clap::Clap;
#[macro_use]
extern crate diesel;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, PgConnection, QueryDsl};
use futures::join;
use futures::stream::StreamExt;
use itertools::Itertools;
use tokio::sync::mpsc;
use tokio::time;
use tokio_diesel::AsyncRunQueryDsl;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

use near_chain_configs::GenesisRecords;

use crate::configs::{Opts, SubCommand};
use crate::db::enums::{AccessKeyAction, ExecutionStatus};
use crate::db::{establish_connection, AccessKey};

mod configs;
mod db;
mod schema;
mod state_viewer;

const INTERVAL: Duration = Duration::from_millis(100);
const INDEXER_FOR_WALLET: &str = "indexer_for_wallet";

/// Map Receipt ID to Execution Outcome
pub type ExecutionOutcomesByReceiptId = HashMap<
    near_indexer::near_primitives::hash::CryptoHash,
    near_indexer::near_primitives::views::ExecutionOutcomeWithIdView,
>;

async fn dump_existing_access_keys(
    home_dir: std::path::PathBuf,
    near_config: near_indexer::NearConfig,
) {
    let (records, latest_block_height) = extract_state_as_genesis_records(home_dir, near_config);
    let pool = establish_connection();
    diesel::delete(schema::access_keys::table)
        .execute_async(&pool)
        .await
        .unwrap();
    insert_access_keys_from_dumped_state(records, latest_block_height, &pool).await;
}

fn extract_state_as_genesis_records(
    home_dir: std::path::PathBuf,
    near_config: near_indexer::NearConfig,
) -> (
    GenesisRecords,
    near_indexer::near_primitives::types::BlockHeight,
) {
    let store = near_store::create_store(&neard::get_store_path(&home_dir));

    let (runtime, state_roots, latest_block_header) = state_viewer::load_trie_stop_at_height(
        store,
        &home_dir,
        &near_config,
        state_viewer::LoadTrieMode::Latest,
    );

    let latest_block_height = latest_block_header.height();
    let dumped_state_genesis = state_viewer::state_dump(
        runtime,
        state_roots,
        latest_block_header,
        &near_config.genesis.config,
    );

    (dumped_state_genesis.records, latest_block_height)
}

async fn insert_access_keys_from_dumped_state(
    records: GenesisRecords,
    height: near_indexer::near_primitives::types::BlockHeight,
    pool: &Pool<ConnectionManager<PgConnection>>,
) {
    let access_keys = records.as_ref().iter().filter_map(|record| {
        if let near_indexer::near_primitives::state_record::StateRecord::AccessKey {
            account_id,
            public_key,
            access_key,
        } = record
        {
            Some(AccessKey {
                public_key: public_key.to_string(),
                account_id: account_id.to_string(),
                action: AccessKeyAction::Add,
                status: ExecutionStatus::Success,
                receipt_hash: "genesis".to_string(),
                block_height: height.into(),
                permission: (&access_key.permission).into(),
            })
        } else {
            None
        }
    });

    let portion_size = 5000;
    let total_access_key_chunks = access_keys.clone().count() / portion_size + 1;
    let access_keys_portion = access_keys.chunks(portion_size);

    let insert_genesis_keys: futures::stream::FuturesUnordered<_> = access_keys_portion
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
                            info!(target: INDEXER_FOR_WALLET, "Trying to push dumped state access keys failed with: {:?}. Retrying in {} seconds...", err, INTERVAL.as_secs_f32());
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
            "Dump state public access keys adding {}%",
            index * 100 / total_access_key_chunks
        );
    }

    info!(
        target: INDEXER_FOR_WALLET,
        "Dumped state public access keys in database successfully replaced."
    );
}

async fn insert_receipts(
    height: near_indexer::near_primitives::types::BlockHeight,
    chunks: &[near_indexer::IndexerChunkView],
    local_receipts: &[near_indexer::near_primitives::views::ReceiptView],
    pool: &Pool<ConnectionManager<PgConnection>>,
    outcomes: &near_indexer::ExecutionOutcomesWithReceipts,
) {
    fn receipt_status(
        outcomes: &near_indexer::ExecutionOutcomesWithReceipts,
        receipt_id: &near_indexer::near_primitives::hash::CryptoHash,
    ) -> Option<db::enums::ExecutionStatus> {
        outcomes.get(receipt_id).map(|execution_outcome| {
            execution_outcome
                .execution_outcome
                .outcome
                .status
                .clone()
                .into()
        })
    }
    let access_keys: Vec<AccessKey> = local_receipts
        .iter()
        .flat_map(|receipt| match receipt.receipt {
            near_indexer::near_primitives::views::ReceiptEnumView::Action { .. } => {
                AccessKey::from_receipt_view(
                    &receipt,
                    height,
                    receipt_status(&outcomes, &receipt.receipt_id),
                )
            }
            _ => vec![],
        })
        .chain(
            chunks
                .iter()
                .flat_map(|chunk| &chunk.receipts)
                .flat_map(|receipt| match receipt.receipt {
                    near_indexer::near_primitives::views::ReceiptEnumView::Action { .. } => {
                        AccessKey::from_receipt_view(
                            receipt,
                            height,
                            receipt_status(&outcomes, &receipt.receipt_id),
                        )
                    }
                    _ => vec![],
                }),
        )
        .collect();

    info!(
        target: INDEXER_FOR_WALLET,
        "Handling receipts related to AccessKey, amount {}",
        access_keys.len()
    );
    if !access_keys.is_empty() {
        loop {
            match diesel::insert_into(schema::access_keys::table)
                .values(access_keys.clone())
                .on_conflict_do_nothing()
                .execute_async(&pool)
                .await
            {
                Ok(_) => break,
                Err(async_error) => {
                    error!(
                        target: INDEXER_FOR_WALLET,
                        "Failed to insert access keys, retrying in {} milliseconds... \n {:#?}",
                        INTERVAL.as_millis(),
                        async_error
                    );
                    time::delay_for(INTERVAL).await;
                }
            };
        }
    }
}

async fn update_receipt_status(
    receipt_ids: Vec<String>,
    status: ExecutionStatus,
    pool: &Pool<ConnectionManager<PgConnection>>,
) {
    debug!(target: INDEXER_FOR_WALLET, "update_receipt_status called");
    if receipt_ids.is_empty() {
        return;
    }

    let rows_touched = loop {
        match diesel::update(
            schema::access_keys::table
                .filter(schema::access_keys::dsl::receipt_hash.eq_any(receipt_ids.clone())),
        )
        .set(schema::access_keys::dsl::status.eq(status))
        .execute_async(pool)
        .await
        {
            Ok(res) => {
                break res;
            }
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
    };
    if rows_touched != receipt_ids.len() {
        warn!(
            target: INDEXER_FOR_WALLET,
            "{} of {} update status [{:?}]",
            rows_touched,
            receipt_ids.len(),
            status
        );
    }
    debug!(target: INDEXER_FOR_WALLET, "update_receipt_status finished");
}

async fn handle_outcomes(
    outcomes: &near_indexer::ExecutionOutcomesWithReceipts,
    pool: &Pool<ConnectionManager<PgConnection>>,
) {
    let mut failed_receipt_ids: Vec<String> = vec![];
    let mut succeeded_receipt_ids: Vec<String> = vec![];

    for outcome in outcomes.values() {
        let status: db::enums::ExecutionStatus =
            outcome.execution_outcome.outcome.status.clone().into();
        match status {
            db::enums::ExecutionStatus::Success => {
                succeeded_receipt_ids.push(outcome.execution_outcome.id.to_string());
            }
            db::enums::ExecutionStatus::Failed => {
                failed_receipt_ids.push(outcome.execution_outcome.id.to_string())
            }
            db::enums::ExecutionStatus::Pending => {
                warn!(
                    target: INDEXER_FOR_WALLET,
                    "ExecutionOutcome status Pending is not expected here ..\n\
                    {:#?}",
                    outcome
                );
            }
        }
    }

    info!(
        target: INDEXER_FOR_WALLET,
        "Saving execution outcomes (Failed amount: {}, Succeeded amount: {})",
        failed_receipt_ids.len(),
        succeeded_receipt_ids.len()
    );

    let update_failed_future =
        update_receipt_status(failed_receipt_ids, ExecutionStatus::Failed, &pool);

    let update_succeeded_future =
        update_receipt_status(succeeded_receipt_ids, ExecutionStatus::Success, &pool);

    join!(update_failed_future, update_succeeded_future);
}

async fn listen_blocks(mut stream: mpsc::Receiver<near_indexer::StreamerMessage>) {
    let pool = establish_connection();

    info!(
        target: INDEXER_FOR_WALLET,
        "NEAR Indexer for Wallet started."
    );

    while let Some(block) = stream.recv().await {
        eprintln!("Block height {:?}", block.block.header.height);

        // Handle receipts
        let insert_receipts_future = insert_receipts(
            block.block.header.height,
            &block.chunks,
            &block.local_receipts,
            &pool,
            &block.receipt_execution_outcomes,
        );

        // Handle outcomes
        info!(
            target: INDEXER_FOR_WALLET,
            "Handling outcomes, total amount {}",
            block.receipt_execution_outcomes.len()
        );
        let handle_outcomes_future = handle_outcomes(&block.receipt_execution_outcomes, &pool);

        join!(insert_receipts_future, handle_outcomes_future);
    }
}

fn main() {
    // We use it to automatically search the for root certificates to perform HTTPS calls
    // (sending telemetry and downloading genesis)
    openssl_probe::init_ssl_cert_env_vars();

    let env_filter = EnvFilter::new(
        "tokio_reactor=info,near=info,near=error,stats=info,telemetry=info,indexer_for_wallet=info,indexer=info",
    );
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();

    let opts: Opts = Opts::parse();

    let home_dir = opts
        .home_dir
        .unwrap_or_else(|| std::path::PathBuf::from(near_indexer::get_default_home()));

    match opts.subcmd {
        SubCommand::Run => {
            let indexer_config = near_indexer::IndexerConfig {
                home_dir,
                sync_mode: near_indexer::SyncModeEnum::FromInterruption,
            };
            let indexer = near_indexer::Indexer::new(indexer_config);
            let stream = indexer.streamer();
            actix::spawn(listen_blocks(stream));
            indexer.start();
        }
        SubCommand::Init(config) => near_indexer::init_configs(
            &home_dir,
            config.chain_id.as_ref().map(AsRef::as_ref),
            config.account_id.as_ref().map(AsRef::as_ref),
            config.test_seed.as_ref().map(AsRef::as_ref),
            config.num_shards,
            config.fast,
            config.genesis.as_ref().map(AsRef::as_ref),
            config.download,
            config.download_genesis_url.as_ref().map(AsRef::as_ref),
        ),
        SubCommand::DumpState => {
            let near_config = neard::load_config(&home_dir);
            actix::run(async move {
                dump_existing_access_keys(home_dir, near_config).await;
                actix::System::current().stop();
            })
            .unwrap();
        }
    }
}
