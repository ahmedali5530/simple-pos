use serde::Serialize;

#[derive(Debug)]
pub enum AppError {
    Db(sqlx::Error),
    BadRequest(String),
    NotFound(String),
    Unauthorized(String),
    Forbidden(String),
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        AppError::Db(value)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Db(err) => write!(f, "{err}"),
            AppError::BadRequest(message)
            | AppError::NotFound(message)
            | AppError::Unauthorized(message)
            | AppError::Forbidden(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: String,
}
