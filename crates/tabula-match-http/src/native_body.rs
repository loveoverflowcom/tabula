//! Fresh current authority through the actual first body-frame handoff.
use super::{
    native_ports::{snapshot_matches, NetworkJournal, QueueOutput},
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
use tabula_core::MatchId;
use tabula_match::{durable::OperationScope, runtime::Binding};
use tabula_session::{
    CredentialOperation, HttpSessionAuthority, SessionError, SessionPublication, SessionSnapshot,
};
use tabula_storage::{
    match_postgres::PgMatchPublication, online_match::PgOnlinePublication,
    session::PgSessionPublication,
};
/// Current storage-owned membership must be checked at the actual delivery boundary.
pub(super) struct PrivateAttachment {
    pub output: Arc<QueueOutput>,
    pub binding: Binding,
    pub journal: Arc<NetworkJournal>,
    pub match_id: MatchId,
    pub scope: OperationScope,
}
enum PrivatePublication {
    Session(Box<PgSessionPublication>),
    Match(Box<PgOnlinePublication>),
}
impl SessionPublication for PrivatePublication {
    fn snapshot(&self) -> &SessionSnapshot {
        match self {
            Self::Session(p) => p.snapshot(),
            Self::Match(p) => p.snapshot(),
        }
    }
    fn publish<T>(
        &mut self,
        action: impl FnOnce(&SessionSnapshot) -> T,
    ) -> Result<T, SessionError> {
        match self {
            Self::Session(p) => p.publish(action),
            Self::Match(p) => p.publish(action),
        }
    }
}
trait OwnerPublication: Unpin {
    fn publish<T>(&mut self, action: impl FnOnce() -> T) -> Result<T, SessionError>;
}
impl OwnerPublication for PgMatchPublication {
    fn publish<T>(&mut self, action: impl FnOnce() -> T) -> Result<T, SessionError> {
        PgMatchPublication::publish(self, action).map_err(|_| SessionError::Unauthenticated)
    }
}
struct GuardedMatchBody<P, O> {
    owner: Option<O>,
    publication: Option<P>,
    bytes: Option<Bytes>,
    attachment: Option<(Arc<QueueOutput>, Binding)>,
}
impl<P: SessionPublication + Unpin, O: OwnerPublication> HttpBody for GuardedMatchBody<P, O> {
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
        let release = || {
            publication
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
                .and_then(|frame| frame.ok_or(SessionError::Unauthenticated))
        };
        let result = match body.owner.as_mut() {
            Some(owner) => owner.publish(release).and_then(std::convert::identity),
            None if body.attachment.is_none() => release(),
            None => Err(SessionError::Unauthenticated),
        };
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
    attachment: Option<PrivateAttachment>,
) -> Response {
    let Ok(bytes) = bounded_json(value) else {
        return unavailable();
    };
    let publication = if let Some(attachment) = &attachment {
        match state
            .online
            .begin_publication(op, attachment.match_id, attachment.scope)
            .await
        {
            Ok(p) => PrivatePublication::Match(Box::new(p)),
            Err(error) => return super::online_problem(error),
        }
    } else {
        match state.sessions.begin_publication(op).await {
            Ok(p) => PrivatePublication::Session(Box::new(p)),
            Err(error) => return session_problem(error),
        }
    };
    let mut owner = if let Some(attachment) = &attachment {
        match attachment.journal.journal.begin_publication().await {
            Ok(owner) => Some(owner),
            Err(_) => return unavailable(),
        }
    } else {
        None
    };
    if let (Some(owner), PrivatePublication::Match(publication)) = (&mut owner, &publication) {
        if owner.restrict_deadline(publication.expires_at()).is_err() {
            return unavailable();
        }
    }
    let mut r = Response::new(Body::new(GuardedMatchBody {
        owner,
        publication: Some(publication),
        bytes: Some(bytes),
        attachment: attachment.map(|attachment| (attachment.output, attachment.binding)),
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tabula_core::{SessionId, UserId};
    use tabula_session::{
        AccountEpoch, AuthSessionId, CredentialDigest, SessionChannel, SessionContextId,
        SessionRecord, SessionSnapshot, UnixMillis,
    };
    struct FakePublication {
        snapshot: SessionSnapshot,
        live: Arc<AtomicBool>,
        once: bool,
        deadline: tokio::time::Instant,
        callbacks: Arc<AtomicUsize>,
    }
    impl SessionPublication for FakePublication {
        fn snapshot(&self) -> &SessionSnapshot {
            &self.snapshot
        }
        fn publish<R>(
            &mut self,
            action: impl FnOnce(&SessionSnapshot) -> R,
        ) -> Result<R, SessionError> {
            if self.once
                || !self.live.load(Ordering::SeqCst)
                || tokio::time::Instant::now() >= self.deadline
            {
                return Err(SessionError::Unauthenticated);
            }
            self.once = true;
            self.callbacks.fetch_add(1, Ordering::SeqCst);
            let result = action(&self.snapshot);
            if !self.live.load(Ordering::SeqCst) || tokio::time::Instant::now() >= self.deadline {
                return Err(SessionError::Unauthenticated);
            }
            Ok(result)
        }
    }
    struct FakeOwner {
        live: Arc<AtomicBool>,
        deadline: tokio::time::Instant,
        pause_after_inner: std::time::Duration,
    }
    impl OwnerPublication for FakeOwner {
        fn publish<T>(&mut self, action: impl FnOnce() -> T) -> Result<T, SessionError> {
            if !self.live.load(Ordering::SeqCst) || tokio::time::Instant::now() >= self.deadline {
                return Err(SessionError::Unauthenticated);
            }
            let result = action();
            std::thread::sleep(self.pause_after_inner);
            if !self.live.load(Ordering::SeqCst) || tokio::time::Instant::now() >= self.deadline {
                return Err(SessionError::Unauthenticated);
            }
            Ok(result)
        }
    }
    fn fixture() -> (
        GuardedMatchBody<FakePublication, FakeOwner>,
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
                owner: Some(FakeOwner {
                    live: Arc::new(AtomicBool::new(true)),
                    deadline: tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                    pause_after_inner: std::time::Duration::ZERO,
                }),
                publication: Some(FakePublication {
                    snapshot,
                    live: live.clone(),
                    once: false,
                    deadline: tokio::time::Instant::now() + std::time::Duration::from_secs(1),
                    callbacks: Arc::new(AtomicUsize::new(0)),
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
    fn non_attachment_body_requires_current_session_but_no_match_owner() {
        let waker = std::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        let (mut body, _, _, _) = fixture();
        body.owner = None;
        body.attachment = None;
        let Poll::Ready(Some(Ok(frame))) = Pin::new(&mut body).poll_frame(&mut cx) else {
            panic!("current session-only metadata must publish without a match owner");
        };
        assert_eq!(
            frame.into_data().unwrap(),
            Bytes::from_static(b"private projection")
        );
        let (mut body, _, _, live) = fixture();
        body.owner = None;
        body.attachment = None;
        live.store(false, Ordering::SeqCst);
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err(SessionError::Unauthenticated)))
        ));
        let (mut body, _, _, _) = fixture();
        body.owner = None;
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err(SessionError::Unauthenticated)))
        ));
    }
    #[test]
    fn independent_owner_loss_and_shorter_nested_deadline_release_no_frame() {
        let waker = std::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        let (mut body, _, _, session_live) = fixture();
        body.owner
            .as_ref()
            .unwrap()
            .live
            .store(false, Ordering::SeqCst);
        assert!(session_live.load(Ordering::SeqCst));
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err(SessionError::Unauthenticated)))
        ));
        let (mut body, _, _, session_live) = fixture();
        let earliest = tokio::time::Instant::now() + std::time::Duration::from_millis(150);
        body.publication.as_mut().unwrap().deadline = earliest;
        let callbacks = body.publication.as_ref().unwrap().callbacks.clone();
        let owner = body.owner.as_mut().unwrap();
        assert!(owner.deadline > earliest);
        owner.deadline = owner.deadline.min(earliest);
        owner.pause_after_inner = std::time::Duration::from_millis(200);
        // The inner session callback succeeds first, then the outer handoff pauses
        // past the shorter shared bound. Its constructed private frame is discarded.
        assert!(matches!(
            Pin::new(&mut body).poll_frame(&mut cx),
            Poll::Ready(Some(Err(SessionError::Unauthenticated)))
        ));
        assert_eq!(
            callbacks.load(Ordering::SeqCst),
            1,
            "shorter nested callback must actually execute before outer expiry"
        );
        assert!(session_live.load(Ordering::SeqCst));
        assert!(body.bytes.is_none());
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
