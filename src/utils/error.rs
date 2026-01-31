use std::fmt;

#[macro_export]
macro_rules! error_data {
    ($func:expr, $source:expr, $message:expr) => {
        ErrorData {
            file: file!(),
            line: line!(),
            func: $func,
            source: $source,
            message: $message,
        }
    };
}

#[derive(Debug)]
pub struct ErrorData {
    pub file: &'static str,
    pub line: u32,
    pub func: &'static str,
    pub source: String,
    pub message: &'static str,
}

impl fmt::Display for ErrorData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} : {} : {}] {}:\n{}",
            self.file, self.line, self.func, self.message, self.source
        )
    }
}
