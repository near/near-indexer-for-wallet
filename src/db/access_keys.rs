use crate::db::enums::{ActionEnum, PermissionEnum, StatusEnum};
use crate::schema;
use bigdecimal::BigDecimal;
use schema::access_keys;

#[derive(Insertable, Queryable, Clone)]
pub(crate) struct AccessKey {
    pub public_key: String,
    pub account_id: String,
    pub action: ActionEnum,
    pub status: StatusEnum,
    pub receipt_hash: String,
    pub block_height: BigDecimal,
    pub permission: PermissionEnum,
}

impl AccessKey {
    pub fn from_receipt_view(
        receipt: &near_indexer::near_primitives::views::ReceiptView,
        block_height: u64,
    ) -> Vec<Self> {
        let mut access_keys: Vec<Self> = vec![];
        if let near_indexer::near_primitives::views::ReceiptEnumView::Action { actions, .. } =
            &receipt.receipt
        {
            for action in actions {
                let access_key = match action {
                    near_indexer::near_primitives::views::ActionView::AddKey {
                        public_key,
                        access_key,
                    } => Self {
                        public_key: public_key.to_string(),
                        account_id: receipt.receiver_id.to_string(),
                        action: ActionEnum::Add,
                        status: StatusEnum::Pending,
                        receipt_hash: receipt.receipt_id.to_string(),
                        block_height: block_height.into(),
                        permission: (&access_key.permission).into(),
                    },
                    near_indexer::near_primitives::views::ActionView::DeleteKey { public_key } => {
                        Self {
                            public_key: public_key.to_string(),
                            account_id: receipt.receiver_id.to_string(),
                            action: ActionEnum::Delete,
                            status: StatusEnum::Pending,
                            receipt_hash: receipt.receipt_id.to_string(),
                            block_height: block_height.into(),
                            permission: PermissionEnum::NotApplicable,
                        }
                    }
                    _ => continue,
                };
                access_keys.push(access_key);
            }
        }
        access_keys
    }
}
