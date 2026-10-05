//! Test-only body scheduler. It delays the actual adapter's private first frame
//! beyond its existing 2-second lease; it never constructs successful auth data.

use std::{
    future::Future as _,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use tokio::time::Sleep;

pub async fn delay_first_private_frame(request: Request, next: Next) -> Response {
    let delayed = request.headers().contains_key("x-test-delay-private-frame")
        && matches!(request.uri().path(), "/api/v1/auth/context" | "/api/v1/me");
    let response = next.run(request).await;
    if !delayed || response.status() != 200 {
        return response;
    }
    let (parts, inner) = response.into_parts();
    Response::from_parts(
        parts,
        Body::new(DelayedBody {
            inner,
            release: Box::pin(tokio::time::sleep(Duration::from_millis(2_100))),
        }),
    )
}

struct DelayedBody {
    inner: Body,
    release: Pin<Box<Sleep>>,
}

impl HttpBody for DelayedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let body = self.get_mut();
        if body.release.as_mut().poll(context).is_pending() {
            return Poll::Pending;
        }
        Pin::new(&mut body.inner).poll_frame(context)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}
