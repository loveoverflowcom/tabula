//! Fresh current authority through the actual first body-frame handoff.
use super::{
    native_ports::{snapshot_matches, QueueOutput},
    session_problem, unavailable, GatewayState,
};
use crate::MAX_RESPONSE_BYTES;
use axum::{
    body::Body,
    http::{header, HeaderValue},
    response::Response,
};
use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde::Serialize;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tabula_match::runtime::Binding;
use tabula_session::{CredentialOperation, SessionError, SessionPublication};
struct GuardedMatchBody<P> {
    publication: Option<P>,
    bytes: Option<Bytes>,
    attachment: Option<(Arc<QueueOutput>, Binding)>,
}
impl<P: SessionPublication + Unpin> HttpBody for GuardedMatchBody<P> {
    type Data = Bytes;
    type Error = SessionError;
    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, SessionError>>> {
        let body = self.get_mut();
        let Some(bytes) = body.bytes.take() else {
            body.publication.take();
            return Poll::Ready(None);
        };
        let Some(mut publication) = body.publication.take() else {
            return Poll::Ready(Some(Err(SessionError::Unavailable)));
        };
        let result = publication
            .publish(|s| {
                if body
                    .attachment
                    .as_ref()
                    .is_some_and(|(output, b)| !snapshot_matches(s, b) || !output.active(b))
                {
                    None
                } else {
                    Some(Frame::data(bytes))
                }
            })
            .and_then(|frame| frame.ok_or(SessionError::Unauthenticated));
        Poll::Ready(Some(result))
    }
    fn is_end_stream(&self) -> bool {
        self.bytes.is_none()
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}
pub(super) async fn private_response(
    state: &GatewayState,
    op: CredentialOperation,
    value: &impl Serialize,
    attachment: Option<(Arc<QueueOutput>, Binding)>,
) -> Response {
    let Ok(bytes) = bounded_json(value) else {
        return unavailable();
    };
    let publication = match state.session_http.begin_publication(op).await {
        Ok(p) => p,
        Err(e) => return session_problem(e),
    };
    let mut r = Response::new(Body::new(GuardedMatchBody {
        publication: Some(publication),
        bytes: Some(bytes),
        attachment,
    }));
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    r
}
fn bounded_json(value: &impl Serialize) -> Result<Bytes, SessionError> {
    let bytes = serde_json::to_vec(value).map_err(|_| SessionError::Unavailable)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(SessionError::Unavailable);
    }
    Ok(Bytes::from(bytes))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tabula_core::{SessionId, UserId};
    use tabula_session::{
        AccountEpoch, AuthSessionId, CredentialDigest, SessionChannel, SessionContextId,
        SessionRecord, SessionSnapshot, UnixMillis,
    };
    struct FakePublication {
        snapshot: SessionSnapshot,
        live: Arc<AtomicBool>,
        once: bool,
    }
    impl SessionPublication for FakePublication {
        fn snapshot(&self) -> &SessionSnapshot {
            &self.snapshot
        }
        fn publish<R>(
            &mut self,
            action: impl FnOnce(&SessionSnapshot) -> R,
        ) -> Result<R, SessionError> {
            if self.once || !self.live.load(Ordering::SeqCst) {
                return Err(SessionError::Unauthenticated);
            }
            self.once = true;
            let result = action(&self.snapshot);
            if !self.live.load(Ordering::SeqCst) {
                return Err(SessionError::Unauthenticated);
            }
            Ok(result)
        }
    }
    fn fixture() -> (
        GuardedMatchBody<FakePublication>,
        Arc<QueueOutput>,
        Binding,
        Arc<AtomicBool>,
    ) {
        let snapshot = SessionRecord::issue(
            AuthSessionId::new(11).unwrap(),
            UserId(22),
            AccountEpoch::new(0).unwrap(),
            SessionChannel::BrowserCookie,
            CredentialDigest::from_bytes([4; 32]),
            SessionContextId::new(33).unwrap(),
            UnixMillis::new(1000).unwrap(),
        )
        .unwrap()
        .snapshot();
        let binding = Binding::new(SessionId(44), UserId(22), 11, 0, 1);
        let output = Arc::new(QueueOutput::default());
        output.insert(binding.clone()).unwrap();
        let live = Arc::new(AtomicBool::new(true));
        (
            GuardedMatchBody {
                publication: Some(FakePublication {
                    snapshot,
                    live: live.clone(),
                    once: false,
                }),
                bytes: Some(Bytes::from_static(b"private projection")),
                attachment: Some((output.clone(), binding.clone())),
            },
            output,
            binding,
            live,
        )
    }
    #[test]
    fn actual_body_poll_checks_current_authority_and_attachment_again() {
        let waker = std::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        let (mut body, _, _, _) = fixture();
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Ok(_)))
        ));
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(None)
        ));
        let (mut body, output, binding, _) = fixture();
        output.remove(&binding);
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err(SessionError::Unauthenticated)))
        ));
        let (mut body, _, _, live) = fixture();
        live.store(false, Ordering::SeqCst);
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err(SessionError::Unauthenticated)))
        ));
    }
    #[test]
    fn complete_body_byte_boundary_is_inclusive_and_oversize_fails_closed() {
        assert_eq!(
            bounded_json(&"A".repeat(MAX_RESPONSE_BYTES - 2))
                .unwrap()
                .len(),
            MAX_RESPONSE_BYTES
        );
        assert!(bounded_json(&"A".repeat(MAX_RESPONSE_BYTES - 1)).is_err());
    }
}
