table! {
    use diesel::sql_types::*;
    use crate::db::enums::*;

    access_keys (public_key, account_id, action, receipt_hash) {
        public_key -> Text,
        account_id -> Text,
        action -> Action_type,
        status -> Status_type,
        receipt_hash -> Text,
        block_height -> Numeric,
        permission -> Permission_type,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::db::enums::*;

    spatial_ref_sys (srid) {
        srid -> Int4,
        auth_name -> Nullable<Varchar>,
        auth_srid -> Nullable<Int4>,
        srtext -> Nullable<Varchar>,
        proj4text -> Nullable<Varchar>,
    }
}

allow_tables_to_appear_in_same_query!(access_keys, spatial_ref_sys,);
