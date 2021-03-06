use std::convert::TryFrom;

use crate::db::enums::{AccessKeyAction, AccessKeyPermission, ExecutionStatus};
use crate::schema;
use bigdecimal::BigDecimal;
use schema::access_keys;

#[derive(Insertable, Queryable, Clone, Debug)]
pub(crate) struct AccessKey {
    pub public_key: String,
    pub account_id: String,
    pub action: AccessKeyAction,
    pub status: ExecutionStatus,
    pub receipt_hash: String,
    pub block_height: BigDecimal,
    pub permission: AccessKeyPermission,
}

impl AccessKey {
    pub fn from_receipt_view(
        receipt: &near_indexer::near_primitives::views::ReceiptView,
        block_height: u64,
        status: Option<ExecutionStatus>,
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
                        action: AccessKeyAction::Add,
                        status: status.unwrap_or_else(|| ExecutionStatus::Pending),
                        receipt_hash: receipt.receipt_id.to_string(),
                        block_height: block_height.into(),
                        permission: (&access_key.permission).into(),
                    },
                    near_indexer::near_primitives::views::ActionView::DeleteKey { public_key } => {
                        Self {
                            public_key: public_key.to_string(),
                            account_id: receipt.receiver_id.to_string(),
                            action: AccessKeyAction::Delete,
                            status: status.unwrap_or_else(|| ExecutionStatus::Pending),
                            receipt_hash: receipt.receipt_id.to_string(),
                            block_height: block_height.into(),
                            permission: AccessKeyPermission::NotApplicable,
                        }
                    }
                    near_indexer::near_primitives::views::ActionView::Transfer { .. } => {
                        if receipt.receiver_id.len() != 64usize {
                            continue;
                        }
                        if let Ok(public_key_bytes) = hex::decode(&receipt.receiver_id) {
                            if let Ok(public_key) =
                                near_crypto::ED25519PublicKey::try_from(&public_key_bytes[..])
                            {
                                Self {
                                    public_key: near_crypto::PublicKey::from(public_key)
                                        .to_string(),
                                    account_id: receipt.receiver_id.to_string(),
                                    action: AccessKeyAction::Add,
                                    status: status.unwrap_or_else(|| ExecutionStatus::Pending),
                                    receipt_hash: receipt.receipt_id.to_string(),
                                    block_height: block_height.into(),
                                    permission: AccessKeyPermission::FullAccess,
                                }
                            } else {
                                continue;
                            }
                        } else {
                            continue;
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
