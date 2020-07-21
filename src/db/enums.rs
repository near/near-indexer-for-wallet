use diesel_derive_enum::DbEnum;
use near_indexer::near_primitives::account::AccessKeyPermission;
use near_indexer::near_primitives::views::AccessKeyPermissionView;

#[derive(Debug, DbEnum, Clone)]
#[DbValueStyle = "SCREAMING_SNAKE_CASE"]
#[DieselType = "Action_type"]
pub enum ActionEnum {
    Add,
    Delete,
}

#[derive(Debug, DbEnum, Clone, Copy)]
#[DbValueStyle = "SCREAMING_SNAKE_CASE"]
#[DieselType = "Status_type"]
pub enum StatusEnum {
    Pending,
    Success,
    Failed,
}

#[derive(Debug, DbEnum, Clone)]
#[DbValueStyle = "SCREAMING_SNAKE_CASE"]
#[DieselType = "Permission_type"]
pub enum PermissionEnum {
    NotApplicable,
    FullAccess,
    FunctionCall,
}

impl From<&AccessKeyPermissionView> for PermissionEnum {
    fn from(item: &AccessKeyPermissionView) -> Self {
        match item {
            AccessKeyPermissionView::FunctionCall { .. } => Self::FunctionCall,
            AccessKeyPermissionView::FullAccess => Self::FullAccess,
        }
    }
}

impl From<&AccessKeyPermission> for PermissionEnum {
    fn from(item: &AccessKeyPermission) -> Self {
        match item {
            AccessKeyPermission::FunctionCall(_) => Self::FunctionCall,
            AccessKeyPermission::FullAccess => Self::FullAccess,
        }
    }
}
