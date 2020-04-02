use actix_http::body::Body;
use actix_http::client::SendRequestError;
use actix_http::http::header;
use actix_http::Response;
use actix_web::error::JsonPayloadError;
use actix_web::http::header::ToStrError;
use actix_web::http::StatusCode;
use actix_web::ResponseError;
use fall_log::*;
use serde::Serialize;
use std::fmt::{Display, Formatter};
use std::io::{Error, ErrorKind};

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum FallError {
    IO_ERROR(Error),
    HTTP_ERROR(StatusCode, Option<Box<dyn std::error::Error>>),
    REMOTE_ERROR(StatusCode, String),
}

impl FallError {
    pub fn remote_err<E: Serialize>(code: u16, e: E) -> Self {
        let sc = StatusCode::from_u16(code).expect("StatusCode Invalid");
        let e = serde_json::to_string(&e).expect("Json encode invalid");
        FallError::REMOTE_ERROR(sc, e)
    }

    pub fn from_err<E: ResponseError>(e: E) -> Self {
        FallError::new(e.status_code(), &format!("{}", e))
    }

    pub fn new(code: StatusCode, err: &str) -> Self {
        FallError::HTTP_ERROR(code, Some(err.into()))
    }

    pub fn bad_request(err: &str) -> Self {
        FallError::new(StatusCode::BAD_REQUEST, err)
    }

    pub fn unauthorized(err: &str) -> Self {
        FallError::new(StatusCode::UNAUTHORIZED, err)
    }
}

impl Display for FallError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            FallError::IO_ERROR(e) => e.fmt(f),
            FallError::HTTP_ERROR(e, o) => {
                if let Some(es) = o {
                    return es.fmt(f);
                }
                e.fmt(f)
            }
            FallError::REMOTE_ERROR(_, o) => o.fmt(f),
        }
    }
}

impl std::error::Error for FallError {}

impl From<FallError> for Error {
    fn from(fe: FallError) -> Self {
        match fe {
            FallError::IO_ERROR(e) => e,
            FallError::HTTP_ERROR(_, e) => {
                error!("{:?}", e);
                ErrorKind::InvalidInput.into()
            }
            FallError::REMOTE_ERROR(e, s) => {
                error!("{:?} - {}", e, s);
                ErrorKind::InvalidData.into()
            }
        }
    }
}

#[derive(Serialize)]
pub struct ErrorBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
    status: u16,
    message: String,
}

impl ResponseError for FallError {
    fn status_code(&self) -> StatusCode {
        match self {
            FallError::IO_ERROR(_) => StatusCode::INTERNAL_SERVER_ERROR,
            FallError::HTTP_ERROR(s, _) => s.clone(),
            FallError::REMOTE_ERROR(s, _) => s.clone(),
        }
    }

    fn error_response(&self) -> Response {
        let mut resp = Response::new(self.status_code());
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        if let FallError::REMOTE_ERROR(_, m) = self {
            return resp.set_body(Body::from(m));
        }
        let body = ErrorBody {
            trace_id: fall_log::current_trace_id(),
            status: resp.status().as_u16(),
            message: format!("{}", self),
        };
        if let Ok(v) = serde_json::to_string(&body) {
            return resp.set_body(Body::from(v));
        }
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        resp
    }
}

impl From<config::ConfigError> for FallError {
    fn from(e: config::ConfigError) -> Self {
        FallError::IO_ERROR(Error::new(ErrorKind::InvalidData, e))
    }
}

impl From<ToStrError> for FallError {
    fn from(e: ToStrError) -> Self {
        FallError::IO_ERROR(Error::new(ErrorKind::InvalidInput, e))
    }
}

impl From<SendRequestError> for FallError {
    fn from(e: SendRequestError) -> Self {
        FallError::from_err(e)
    }
}

impl From<JsonPayloadError> for FallError {
    fn from(e: JsonPayloadError) -> Self {
        FallError::from_err(e)
    }
}

impl From<awc::error::JsonPayloadError> for FallError {
    fn from(e: awc::error::JsonPayloadError) -> Self {
        FallError::from_err(e)
    }
}

#[cfg(feature = "r2d2")]
impl From<r2d2::Error> for FallError {
    fn from(e: r2d2::Error) -> Self {
        FallError::IO_ERROR(Error::new(ErrorKind::InvalidInput, e))
    }
}

#[cfg(feature = "database")]
impl From<diesel::result::Error> for FallError {
    fn from(e: diesel::result::Error) -> Self {
        FallError::IO_ERROR(Error::new(ErrorKind::InvalidData, e))
    }
}

#[cfg(feature = "redis")]
impl From<r2d2_redis::redis::RedisError> for FallError {
    fn from(e: r2d2_redis::redis::RedisError) -> Self {
        FallError::IO_ERROR(Error::new(ErrorKind::InvalidData, e))
    }
}
