use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter, self, Debug};
use std::io::{self, ErrorKind};
use std::marker::PhantomData;
use std::string::ParseError;
use crate::message::{Response, Request};

pub const DEFAULT_HANDLER: fn(&Request, err: DefaultError) -> Response = |req, err| {
    eprintln!("Error: {}", err);

    match err {
        DefaultError::NotFound => Response::text("Not found", 404),
        DefaultError::RequestParse(_) => Response::text("Malformed request", 500),
        DefaultError::Other(_) => Response::text("Internal server error", 500)
    }
};

pub trait ServerError: Error + Sync + Send {}

#[derive(Debug, Copy, Clone)]
pub struct InvalidMethodError;

impl Display for InvalidMethodError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "The given http method is invalid")
    }
}

impl Error for InvalidMethodError {}

#[derive(Debug)]
pub enum RequestParseError {
    MalformedRequest,
    Method,
    Route,
    Protocol,
    Host,
    Body,
    Header(String)
}

impl Display for RequestParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse {}", format!("{:?}", self).to_lowercase())
    }
}

impl Error for RequestParseError {}

impl From<InvalidMethodError> for RequestParseError {
    fn from(_: InvalidMethodError) -> Self {
        RequestParseError::Method
    }
}

#[derive(Debug)]
pub enum DefaultError {
    NotFound,
    RequestParse(RequestParseError),
    Other(Box<dyn Error + Send + Sync>)
}

impl Display for DefaultError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Could not find the requested resource"),
            Self::RequestParse(err) => write!(f, "Failed to parse request"),
            Self::Other(err) => write!(f, "Internal server error. {}", err.to_string())
        }
    }
}

impl Error for DefaultError {}
impl ServerError for DefaultError {}

impl From<io::Error> for DefaultError {
    fn from(err: io::Error) -> DefaultError {
        match err.kind() {
            ErrorKind::NotFound => Self::NotFound,
            _ => Self::Other(Box::new(err))
        }
    }
}

impl From<serde_json::Error> for DefaultError {
    fn from(err: serde_json::Error) -> DefaultError {
        Self::Other(Box::new(err))
    }
}

impl From<RequestParseError> for DefaultError {
    fn from(err: RequestParseError) -> DefaultError {
        Self::Other(Box::new(err))
    }
}

pub trait ErrorAction<E: ServerError> : Fn(&Request, E) -> Response + Sync + Send + 'static {}
impl <E: ServerError, F: Fn(&Request, E) -> Response + Sync + Send + 'static> ErrorAction<E> for F {}