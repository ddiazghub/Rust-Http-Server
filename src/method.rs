use crate::error::InvalidMethodError;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete
}

impl TryFrom<&str> for HttpMethod {
    type Error = InvalidMethodError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "PATCH" => Ok(HttpMethod::Patch),
            "DELETE" => Ok(HttpMethod::Delete),
            _ => Err(InvalidMethodError)
        }
    }
}