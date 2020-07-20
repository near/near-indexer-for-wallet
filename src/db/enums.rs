use diesel_derive_enum::DbEnum;

#[derive(Debug, DbEnum, Clone)]
#[DbValueStyle = "SCREAMING_SNAKE_CASE"]
#[DieselType = "Action_type"]
pub enum ActionEnum {
    Add,
    Delete,
}

#[derive(Debug, DbEnum, Clone)]
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