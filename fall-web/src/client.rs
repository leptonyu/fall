use actix_http::http::Method;
use actix_http::http::Uri;
use actix_http::RequestHead;
use actix_web::client::Client;
use actix_web::client::ClientRequest;
use awc::error::HttpError;
use awc::ws;
use fall_log::next_open_trace;
use std::convert::TryFrom;

#[derive(Clone)]
pub struct FallClient {
    client: Client,
    opentracing: bool,
}

fn set_trace(enable: bool, req: ClientRequest) -> ClientRequest {
    if enable {
        if let Some(s) = next_open_trace() {
            return req
                .header("X-B3-TraceId", s.trace_id)
                .header("X-B3-SpanId", s.span_id)
                .header("X-B3-ParentSpanId", s.parent_span_id);
        }
    }
    req
}

pub trait ClientRequestExt {
    fn accept_json(self) -> Self;
}

impl ClientRequestExt for ClientRequest {
    fn accept_json(self) -> Self {
        self.header("Content-Type", "application/json")
    }
}

impl FallClient {
    pub fn new() -> Self {
        FallClient {
            client: Client::new(),
            opentracing: true,
        }
    }

    pub fn raw_client(&self) -> &Client {
        &self.client
    }

    pub fn request<U>(&self, method: Method, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        set_trace(self.opentracing, self.client.request(method, url))
    }

    pub fn request_from<U>(&self, url: U, head: &RequestHead) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        set_trace(self.opentracing, self.client.request_from(url, head))
    }

    pub fn get<U>(&self, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.request(Method::GET, url)
    }

    /// Construct HTTP *HEAD* request.
    pub fn head<U>(&self, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.request(Method::HEAD, url)
    }

    /// Construct HTTP *PUT* request.
    pub fn put<U>(&self, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.request(Method::PUT, url)
    }

    /// Construct HTTP *POST* request.
    pub fn post<U>(&self, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.request(Method::POST, url)
    }

    /// Construct HTTP *PATCH* request.
    pub fn patch<U>(&self, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.request(Method::PATCH, url)
    }

    /// Construct HTTP *DELETE* request.
    pub fn delete<U>(&self, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.request(Method::DELETE, url)
    }

    /// Construct HTTP *OPTIONS* request.
    pub fn options<U>(&self, url: U) -> ClientRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.request(Method::OPTIONS, url)
    }

    /// Construct WebSockets request.
    pub fn ws<U>(&self, url: U) -> ws::WebsocketsRequest
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<HttpError>,
    {
        self.client.ws(url)
    }
}
