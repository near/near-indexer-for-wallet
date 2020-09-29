use diesel_derive_enum::DbEnum;

use near_indexer::near_primitives;

#[derive(Debug, DbEnum, Clone)]
#[DbValueStyle = "SCREAMING_SNAKE_CASE"]
#[DieselType = "Access_key_action_type"]
#[PgType = "access_key_action_type"]
pub enum AccessKeyAction {
    Add,
    Delete,
}

#[derive(Debug, DbEnum, Clone, Copy)]
#[DbValueStyle = "SCREAMING_SNAKE_CASE"]
#[DieselType = "Execution_status_type"]
#[PgType = "execution_status_type"]
pub enum ExecutionStatus {
    Pending,
    Success,
    Failed,
}

impl From<near_primitives::views::ExecutionStatusView> for ExecutionStatus {
    fn from(status_view: near_primitives::views::ExecutionStatusView) -> Self {
        match status_view {
            // Assume Unknown status as Failed for indexer
            near_primitives::views::ExecutionStatusView::Failure(_)
            | near_primitives::views::ExecutionStatusView::Unknown => Self::Failed,
            near_primitives::views::ExecutionStatusView::SuccessReceiptId(_)
            | near_primitives::views::ExecutionStatusView::SuccessValue(_) => Self::Success,
        }
    }
}

#[derive(Debug, DbEnum, Clone)]
#[DbValueStyle = "SCREAMING_SNAKE_CASE"]
#[DieselType = "Access_key_permission_type"]
#[PgType = "access_key_permission_type"]
pub enum AccessKeyPermission {
    /// Used only with AccessKeyAction::Delete
    NotApplicable,
    /// Used only with AccessKeyAction::Add
    FullAccess,
    /// Used only with AccessKeyAction::Add
    FunctionCall,
}

impl From<&near_primitives::views::AccessKeyPermissionView> for AccessKeyPermission {
    fn from(item: &near_primitives::views::AccessKeyPermissionView) -> Self {
        match item {
            near_primitives::views::AccessKeyPermissionView::FunctionCall { .. } => {
                Self::FunctionCall
            }
            near_primitives::views::AccessKeyPermissionView::FullAccess => Self::FullAccess,
        }
    }
}

impl From<&near_primitives::account::AccessKeyPermission> for AccessKeyPermission {
    fn from(item: &near_primitives::account::AccessKeyPermission) -> Self {
        match item {
            near_primitives::account::AccessKeyPermission::FunctionCall(_) => Self::FunctionCall,
            near_primitives::account::AccessKeyPermission::FullAccess => Self::FullAccess,
        }
    }
}
