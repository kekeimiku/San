#[derive(Debug)]
pub struct Error(pub String);

impl From<ptrsx::error::Error> for Error {
    fn from(value: ptrsx::error::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self(value.to_string())
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(value: std::num::ParseIntError) -> Self {
        Self(value.to_string())
    }
}
