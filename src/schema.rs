table! {
    use diesel::sql_types::*;
    use crate::db::enums::*;

    access_keys (public_key, account_id, action, receipt_hash) {
        public_key -> Text,
        account_id -> Text,
        action -> AccessKeyActionMapping,
        status -> ExecutionStatusMapping,
        receipt_hash -> Text,
        block_height -> Numeric,
        permission -> AccessKeyPermissionMapping,
    }
}
