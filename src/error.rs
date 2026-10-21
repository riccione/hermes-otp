// Exit codes are part of the public contract: scripts rely on them.
//
//   0  success
//   1  runtime error (also: no code could be produced)
//   2  usage error, owned by clap
//   3  partial success (some records produced a code, others did not)

pub const EXIT_ERROR: i32 = 1;
pub const EXIT_PARTIAL: i32 = 3;

pub struct AppError {
    pub message: String,
    pub exit_code: i32,
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        AppError {
            message,
            exit_code: EXIT_ERROR,
        }
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        AppError {
            message: message.to_string(),
            exit_code: EXIT_ERROR,
        }
    }
}

impl AppError {
    pub fn partial(message: impl Into<String>) -> Self {
        AppError {
            message: message.into(),
            exit_code: EXIT_PARTIAL,
        }
    }
}
