# NEAR Indexer for Wallet

NEAR Indexer for Wallet is built on top of [NEAR Indexer microframework](https://github.com/nearprotocol/nearcore/tree/master/chain/indexer) to watch the network and store all the AccessKeys events in the PostgreSQL database. 
It relies on receipts and `ExecutionOutcomes` to expose the action (`ADD` or `DELETE`) and the status of this action (`SUCCESS` or `FAILED`)

### AccessKeys database structure

See the [migration](https://github.com/near/near-indexer-for-wallet/blob/master/migrations/2020-07-15-154433_create_access_keys/up.sql) 
to learn about the structure.

To query the `public_key` for specific `account_id` to define if the key is present do:

```sql
SELECT "action"
FROM access_keys
WHERE public_key = :public_key AND account_id = :account_id AND status = 'SUCCESS'
ORDER BY block_height DESC
LIMIT 1
``` 

So if the last `"action"` is `ADD` then the `public_key` exists. If the `"action"` is `DELETE` than it doesn't exist anymore.

## Getting started

Before you proceed, make sure you have the following software installed:
* [rustup](https://rustup.rs/) or Rust version that is mentioned in `rust-toolchain` file in the root of [nearcore](https://github.com/nearprotocol/nearcore) project.

Clone this repository and open the project folder

```bash
$ git clone git@github.com:near/near-indexer-for-wallet.git
$ cd near-indexer-for-wallet
```

You need to provide database credentials in `.env` file like below (replace `user`, `password`, `host` and `db_name` with yours):

```bash
$ echo "DATABASE_URL=postgres://user:password@host/db_name" > .env
```

Then you need to apply migrations to create necessary database structure, for this you'll need `diesel-cli`, you can install it like so:

```bash
$ cargo install diesel_cli --no-default-features --features "postgres"
```

And apply migrations

```bash
$ diesel migation run
```

To connect NEAR Indexer for Wallet to the specific chain you need to have necessary configs, you can generate it as follows:

```bash
$ cargo run --release -- --home-dir ~/.near/testnet init --chain-id testnet --download
```

Replace `testnet` in the command above to choose different chain: `betanet` or `mainnet`. 
This will generate keys and configs and download official genesis config.

Configs for the specified network are in the `--home-dir` provided folder. We need to ensure that NEAR Indexer for Wallet follows 
all the necessary shards, so `"tracked_shards"` parameters in `~/.near/testnet/config.json` needs to be configured properly. 
For example, with a single shared network, you just add the shard #0 to the list:

```
...
"tracked_shards": [0],
...
```

To run NEAR Indexer for Wallet:

```bash
$ cargo run --release -- --home-dir ~/.near/testnet run
```

After the network is synced, you should see logs of every block height currently received by NEAR Indexer for Wallet. 

## Initial AccessKeys to database

**NB!** This is a workaround to get the proper up to date data. This may change once `nearcore` allow to simplify this process. 

In order to collect the AccessKeys from current state into database (e.g. at the first start of the NEAR Indexer for Wallet node) 
you need to wait until the node has synced the data and then stop it. Run the `dump-state` command and then start the node again. 

```bash
$ cargo run --release -- --home-dir ~/.near/testnet dump-state
```

It shouldn't take long, you'll see the message "Dumped state public access keys in database successfully replaced." after that start the node again

```bash
$ cargo run --release -- --home-dir ~/.near/testnet run
```