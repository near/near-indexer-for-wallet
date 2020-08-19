use std::time::Duration;

use clap::derive::Clap;
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


async fn access_keys_from_state(home_dir: std::path::PathBuf, near_config: near_indexer::NearConfig) {
    let store = near_store::create_store(&neard::get_store_path(&home_dir));

    let (runtime, state_roots, header) =
        state_viewer::load_trie_stop_at_height(store, &home_dir, &near_config, state_viewer::LoadTrieMode::Latest);

    let height = header.height();
    let new_genesis =
        state_viewer::state_dump(runtime, state_roots.clone(), header, &near_config.genesis.config);

    handle_genesis_public_keys(new_genesis.records, height).await;
}

async fn handle_genesis_public_keys(records: GenesisRecords, height: u64) {
    let pool = establish_connection();
    let access_keys = records
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

    let chunk_size = 5000;
    let total_access_key_chunks = access_keys.clone().count() / chunk_size + 1;
    let slice = access_keys.chunks(chunk_size);

    diesel::delete(schema::access_keys::table).execute_async(&pool).await.unwrap();

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
    status: ExecutionStatus,
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

    info!(
        target: INDEXER_FOR_WALLET,
        "NEAR Indexer for Wallet started."
    );

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
                .on_conflict_do_nothing()
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
        update_receipt_status(failed_receipt_ids, ExecutionStatus::Failed, &pool).await;

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
        update_receipt_status(succeeded_receipt_ids, ExecutionStatus::Success, &pool).await;
    }
}

fn main() {
    // We use it to automatically search the for root certificates to perform HTTPS calls
    // (sending telemetry and downloading genesis)
    openssl_probe::init_ssl_cert_env_vars();

    let env_filter = EnvFilter::new(
        "tokio_reactor=info,near=info,near=error,stats=info,telemetry=info,indexer_for_wallet=info",
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
            let indexer = near_indexer::Indexer::new(Some(&home_dir));
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
                access_keys_from_state(home_dir, near_config).await;
                actix::System::current().stop();
            }).unwrap();
        }
    }
}
