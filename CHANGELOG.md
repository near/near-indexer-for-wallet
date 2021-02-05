# Changelog

## 1.3.0

* Upgrade `nearcore` dependency
* Now runtime is being created in the Indexer not in the Indexer Framework (0.8.0)
* Migrate from `tokio-diesel` to `actix-diesel` after upgrade `tokio` and `actix` dependency

## 1.2.1

* Upgrade `nearcore` dependency

## 1.2.0

* Upgrade `nearcore` dependency that includes NEAR Indexer Framework of version 0.7.0
with bunch of breaking changes
* Update the flow according to that changes
* Limit spawned processes to 100 by using buffer_unordered

## 1.1.3

* Upgrade `nearcore` dependency

## 1.1.2

* Handle implicit accounts via `Transfer` receipt and 64 length `receiver_id`

## 1.1.1

* Update NEAR Indexer framework dependency to newer commit hash
* Add `From` trait to `db::enums::ExecutionStatus` from `near_primitives::views::ExecutionStatusView`
* Update `db::access_keys::AccessKey::from_receipt_view` with optional arg `status` to be able to set status of the record at once (useful for local receipts)
* Add warning to `update_receipt_status` to keep track on what is going on there
* Refactor `handle_outcomes` a bit to use `From` trait for `ExecutionStatus` and clean the code a little bit

## 1.1.0

* Update dependency of NEAR Indexer framework to version `0.2.0`
* Handle `local_receipts`
* Update handling `ExecutionOutcome` according to changes in `0.2.0` NEAR Indexer framework

## 1.0.0

* Added `dump-state` command
* Described the `dump-state` in the README.md
