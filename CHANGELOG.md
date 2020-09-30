# Changelog

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
