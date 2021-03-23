use std::collections::HashMap;
use std::sync::Arc;

use near_chain::{ChainStore, ChainStoreAccess, RuntimeAdapter};
use near_chain_configs::{Genesis, GenesisConfig};
use near_indexer::near_primitives::block_header::BlockHeader;
use near_indexer::near_primitives::state_record::StateRecord;
use near_indexer::near_primitives::types::{AccountInfo, BlockHeight, StateRoot};
use near_store::{Store, TrieIterator};
use neard::NightshadeRuntime;

#[allow(unused)]
pub(crate) enum LoadTrieMode {
    /// Load latest state
    Latest,
    /// Load prev state at some height
    Height(BlockHeight),
    /// Load the prev state of the last final block from some height
    LastFinalFromHeight(BlockHeight),
}

pub(crate) fn load_trie_stop_at_height(
    store: Arc<Store>,
    home_dir: &std::path::Path,
    near_config: &near_indexer::NearConfig,
    mode: LoadTrieMode,
) -> (NightshadeRuntime, Vec<StateRoot>, BlockHeader) {
    let mut chain_store = ChainStore::new(store.clone(), near_config.genesis.config.genesis_height);

    let runtime = NightshadeRuntime::new(
        &home_dir,
        store,
        &near_config.genesis,
        near_config.client_config.tracked_accounts.clone(),
        near_config.client_config.tracked_shards.clone(),
    );
    let head = chain_store.head().unwrap();
    let last_block = match mode {
        LoadTrieMode::LastFinalFromHeight(height) => {
            // find the first final block whose height is at least `height`.
            let mut cur_height = height + 1;
            loop {
                if cur_height >= head.height {
                    panic!("No final block with height >= {} exists", height);
                }
                let cur_block_hash = match chain_store.get_block_hash_by_height(cur_height) {
                    Ok(hash) => hash,
                    Err(_) => {
                        cur_height += 1;
                        continue;
                    }
                };
                let last_final_block_hash = *chain_store
                    .get_block_header(&cur_block_hash)
                    .unwrap()
                    .last_final_block();
                let last_final_block = chain_store.get_block(&last_final_block_hash).unwrap();
                if last_final_block.header().height() >= height {
                    break last_final_block.clone();
                } else {
                    cur_height += 1;
                    continue;
                }
            }
        }
        LoadTrieMode::Height(height) => {
            let block_hash = chain_store.get_block_hash_by_height(height).unwrap();
            chain_store.get_block(&block_hash).unwrap().clone()
        }
        LoadTrieMode::Latest => chain_store
            .get_block(&head.last_block_hash)
            .unwrap()
            .clone(),
    };
    let state_roots = last_block
        .chunks()
        .iter()
        .map(|chunk| chunk.prev_state_root())
        .collect();
    (runtime, state_roots, last_block.header().clone())
}

pub(crate) fn state_dump(
    runtime: NightshadeRuntime,
    state_roots: Vec<StateRoot>,
    last_block_header: BlockHeader,
    genesis_config: &GenesisConfig,
) -> Genesis {
    println!(
        "Generating genesis from state data of #{} / {}",
        last_block_header.height(),
        last_block_header.hash()
    );
    let genesis_height = last_block_header.height() + 1;
    let block_producers = runtime
        .get_epoch_block_producers_ordered(&last_block_header.epoch_id(), last_block_header.hash())
        .unwrap();
    let validators = block_producers
        .into_iter()
        .filter_map(|(info, is_slashed)| {
            if !is_slashed {
                Some((info.account_id, (info.public_key, info.stake)))
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>();

    let mut records = vec![];
    for (shard_id, state_root) in state_roots.iter().enumerate() {
        let trie = runtime.get_trie_for_shard(shard_id as u64);
        let trie = TrieIterator::new(&trie, &state_root).unwrap();
        for item in trie {
            let (key, value) = item.unwrap();
            if let Some(mut sr) = StateRecord::from_raw_key_value(key, value) {
                if let StateRecord::Account {
                    account_id,
                    account,
                } = &mut sr
                {
                    if account.locked() > 0 {
                        let stake = *validators.get(account_id).map(|(_, s)| s).unwrap_or(&0);
                        account.set_amount(account.amount() + account.locked() - stake);
                        account.set_locked(stake);
                    }
                }
                records.push(sr);
            }
        }
    }

    let mut genesis_config = genesis_config.clone();
    genesis_config.genesis_height = genesis_height;
    genesis_config.validators = validators
        .into_iter()
        .map(|(account_id, (public_key, amount))| AccountInfo {
            account_id,
            public_key,
            amount,
        })
        .collect();
    Genesis::new(genesis_config, records.into())
}
