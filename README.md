# NEAR Indexer for Wallet

The indexer for NEAR Wallet uses [near-indexer](https://github.com/nearprotocol/nearcore/tree/master/chain/indexer) 
to listen the network. Syncing all the `AddKey` and `DeleteKey` actions from successful receipts, 
storing records with `public_key` and `account_id` into database (PostgreSQL). Updates the status of 
the record once `ExecutionOutcome` is received.

 ## How to set up and run NEAR Indexer for Wallet
 
 Before you proceed, make sure you have the following software installed:
 * [rustup](https://rustup.rs/) or Rust version that is mentioned in `rust-toolchain` file in the 
 root of nearcore project.
 
 Also you will need network configs and node keys
 
 Clone NEAR Indexer for Wallet repo
 
 ```bash
$ git clone git@github.com:near/near-indexer-for-wallet.git
$ cd near-indexer-for-wallet
 ```

Build NEAR Indexer for Wallet

Create `.env` file in the root with your database credentials

```bash
$ echo "DATABASE_URL=postgres://user:password@host/database_name" > .env
```

```bash
$ cargo run --release
```
