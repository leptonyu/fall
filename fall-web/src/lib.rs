use actix_service::Service;
use actix_service::Transform;
use actix_web::body::MessageBody;
use actix_web::dev::ServiceRequest;
use actix_web::dev::ServiceResponse;
use actix_web::Error;
use futures_core::future::LocalBoxFuture;
use futures_util::future;
use futures_util::future::FutureExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use fall_log::*;

pub struct OpenTrace {
    trace_id: String,
    span_id: String,
    parent_span_id: String,
}

fn read_header_as_u64(name: &str, req: &ServiceRequest) -> Option<u64> {
    req.headers()
        .get(name)
        .and_then(|r| r.to_str().ok())
        .and_then(|r| r.parse().ok())
}

impl OpenTrace {
    pub fn new(req: &ServiceRequest) -> Self {
        let trace_id = format!(
            "{:x}",
            match read_header_as_u64("X-B3-TraceId", req) {
                Some(v) => v,
                _ => rand::random::<u64>(),
            }
        );
        let span_id = match read_header_as_u64("X-B3-SpanId", req) {
            Some(v) => format!("{:x}", v),
            _ => trace_id.clone(),
        };
        OpenTrace {
            trace_id,
            parent_span_id: read_header_as_u64("X-B3-ParentSpanId", req)
                .map(|r| format!("{:x}", r))
                .unwrap_or("".into()),
            span_id,
        }
    }
}

pub trait RequestHandler {
    fn new_span(&self, req: &ServiceRequest) -> span::Span {
        let ot = OpenTrace::new(req);
        span!(
            Level::INFO,
            "web",
            trace_id = display(ot.trace_id),
            span_id = display(ot.span_id),
            parent_span_id = display(ot.parent_span_id),
        )
    }

    fn pre_request(&self, req: &ServiceRequest) -> future::Ready<Result<(), actix_web::Error>>;
}

pub struct DefaultRequestHandler;

impl RequestHandler for DefaultRequestHandler {
    fn pre_request(&self, _req: &ServiceRequest) -> future::Ready<Result<(), actix_web::Error>> {
        future::ok(())
    }
}

pub struct FallTransform<H>
where
    H: RequestHandler,
{
    handler: Rc<H>,
}

impl<H> FallTransform<H>
where
    H: RequestHandler,
{
    pub fn new(handler: H) -> Self {
        FallTransform {
            handler: Rc::new(handler),
        }
    }
}

impl Default for FallTransform<DefaultRequestHandler> {
    fn default() -> Self {
        FallTransform::new(DefaultRequestHandler)
    }
}

pub struct FallMiddleware<S, H>
where
    H: RequestHandler,
{
    service: Rc<RefCell<S>>,
    handler: Rc<H>,
}

impl<S, H, B> Transform<S> for FallTransform<H>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S: 'static,
    B: MessageBody,
    H: RequestHandler + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type InitError = ();
    type Error = actix_web::Error;
    type Transform = FallMiddleware<S, H>;
    type Future = future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        future::ok(FallMiddleware {
            service: Rc::new(RefCell::new(service)),
            handler: self.handler.clone(),
        })
    }
}

impl<S, H, B> Service for FallMiddleware<S, H>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S: 'static,
    B: MessageBody,
    H: RequestHandler + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }
    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let mut sv = self.service.clone();
        let hd = self.handler.clone();
        async move {
            let span = hd.new_span(&req);
            let _enter = span.enter();
            match hd.pre_request(&req).await {
                Ok(()) => sv.call(req).await,
                Err(e) => Ok(req.error_response(e)),
            }
        }
        .boxed_local()
    }
}
