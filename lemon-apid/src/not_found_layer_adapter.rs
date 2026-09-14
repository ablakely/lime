use std::{convert::Infallible, pin::Pin};

use axum::response::IntoResponse;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type Request = axum::extract::Request<()>;
type Response = axum::response::Response;

#[derive(Clone)]
pub struct NotFoundLayerAdapter<S> {
    service: S,
}

impl<S> NotFoundLayerAdapter<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct NotFoundServiceAdapter<S, FallbackService> {
    upper_service: S,
    fallback_service: FallbackService,
}

impl<S, FallbackService> tower::Layer<FallbackService> for NotFoundLayerAdapter<S>
where
    S: Clone,
{
    type Service = NotFoundServiceAdapter<S, FallbackService>;

    fn layer(&self, fallback_service: FallbackService) -> Self::Service {
        NotFoundServiceAdapter {
            upper_service: self.service.clone(),
            fallback_service,
        }
    }
}

impl<S, FallbackService> tower::Service<axum::extract::Request>
    for NotFoundServiceAdapter<S, FallbackService>
where
    S: tower::Service<Request, Error = Infallible> + Send + Clone + 'static,
    S::Future: Send,
    S::Response: axum::response::IntoResponse,
    FallbackService: tower::Service<axum::extract::Request, Error = Infallible, Response = Response>
        + Send
        + Clone
        + 'static,
    FallbackService::Future: Send,
    FallbackService::Response: axum::response::IntoResponse,
{
    type Response = Response;
    type Error = Infallible;
    type Future = BoxFuture<std::result::Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if self.upper_service.poll_ready(cx).is_ready()
            && self.fallback_service.poll_ready(cx).is_ready()
        {
            std::task::Poll::Ready(Ok(()))
        } else {
            std::task::Poll::Pending
        }
    }

    fn call(&mut self, req: axum::extract::Request) -> Self::Future {
        let (head, body) = req.into_parts();
        let min_req = Request::from_parts(head.clone(), ());
        let req = axum::extract::Request::from_parts(head, body);
        let mut upper_service = self.upper_service.clone();
        let mut fallback_service = self.fallback_service.clone();
        Box::pin(async move {
            let upper_response = upper_service.call(min_req).await.unwrap().into_response();
            if upper_response.status() == axum::http::StatusCode::NOT_FOUND {
                Ok(fallback_service.call(req).await?.into_response())
            } else {
                Ok(upper_response)
            }
        })
    }
}
