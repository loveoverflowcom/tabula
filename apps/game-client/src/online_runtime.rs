//! Browser transport glue around the existing game-generic Macroquad presenter.
//! Fetch futures are polled once per frame, never inside presentation.
use super::{notify_runtime_ready, play_cues, presentation_now_ms, SetViewport};
use macroquad::prelude as mq;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use tabula_game_api::{GameModule, GameRules};
use tabula_game_client::{
    fixture_assets::{LocalAssetScene, LocalSpriteResources},
    online::OnlineMatch,
    resolve_display_geometry,
};
use tabula_match_http::{MatchAttachment, MatchFrames};
use tabula_net_client::direct::DirectState;
use tabula_presentation::{FrameCtx, GamePresentation, Renderer};
use tabula_render_macroquad::{MacroquadAudioSink, MacroquadRenderer};

type ResponseFuture = Pin<Box<dyn Future<Output = Result<Vec<u8>, macroquad::Error>>>>;
fn request(name: String) -> ResponseFuture {
    Box::pin(async move { mq::load_file(&name).await })
}
fn poll_request(future: &mut ResponseFuture) -> Poll<Result<Vec<u8>, macroquad::Error>> {
    future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialResponse {
    game_id: String,
    game_version: String,
    attachment: MatchAttachment,
    pending: Option<PendingResponse>,
    pending_unknown: bool,
    transport_generation: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingResponse {
    operation_scope: String,
    command: String,
}
#[derive(serde::Deserialize)]
#[serde(tag = "transport", rename_all = "lowercase", deny_unknown_fields)]
enum TransportResponse {
    Recovering,
    Resync { bootstrap: InitialResponse },
}

/// One online loop, holding projections and local presentation only.
#[allow(clippy::too_many_lines, clippy::float_arithmetic)] // Renderer-only geometry, never authoritative.
pub(super) async fn run_online<M, P>(
    renderer: &mut MacroquadRenderer,
    audio: &mut MacroquadAudioSink,
    theme: &tabula_design::Theme,
    match_id: &str,
    reduced_motion: bool,
    resources: &LocalSpriteResources,
) where
    M: GameModule,
    P: GamePresentation<Rules = M::Rules>,
    P::Local: SetViewport,
    <M::Rules as GameRules>::View: serde::de::DeserializeOwned,
    <M::Rules as GameRules>::ViewEvent: serde::de::DeserializeOwned,
{
    let initial = match mq::load_file("tabula-online-attach.txt").await {
        Ok(bytes) if bytes.len() <= tabula_match_http::MAX_RESPONSE_BYTES => {
            serde_json::from_slice::<InitialResponse>(&bytes)
                .map_err(|_| "The server attachment is incompatible")
        }
        _ => Err("The online attachment is unavailable"),
    };
    let Ok(initial) = initial else {
        unavailable().await;
        return;
    };
    let Ok(id) = tabula_match_http::parse_match_id(match_id) else {
        unavailable().await;
        return;
    };
    let metadata = M::metadata();
    if initial.game_id != metadata.id().as_str()
        || initial.game_version != metadata.version().as_str()
    {
        unavailable().await;
        return;
    }
    let restored = initial.pending;
    let pending_unknown = initial.pending_unknown;
    let mut host_generation = initial.transport_generation;
    let initial = initial.attachment;
    let Ok(mut online) = OnlineMatch::<M::Rules, P>::new(
        id,
        metadata.id().clone(),
        metadata.version().clone(),
        initial.next_seq(),
        initial.frames(),
    ) else {
        unavailable().await;
        return;
    };
    if online.bind_scope(initial.operation_scope()).is_err() {
        unavailable().await;
        return;
    }
    let mut restored_command = None;
    if let Some(pending) = restored {
        if pending.command.len() > tabula_match_http::MAX_REQUEST_BYTES {
            unavailable().await;
            return;
        }
        match serde_json::from_str::<tabula_protocol::ClientEnvelope>(&pending.command) {
            Ok(command) => {
                let Ok(value) = online.restore_pending(&pending.operation_scope, command) else {
                    unavailable().await;
                    return;
                };
                restored_command = value;
            }
            Err(_) => online.mark_unknown(),
        }
    }
    if pending_unknown {
        online.mark_unknown();
        restored_command = None;
    }
    if online.state() == DirectState::ReadOnly {
        let _ = mq::load_file("tabula-online-unknown.txt").await;
    }
    online.local_mut().set_reduced_motion(reduced_motion);
    let mut inflight = None::<ResponseFuture>;
    let mut command = restored_command;
    let mut recover_request = false;
    let mut recovery_cycles = 0u8;
    let mut pending_since = None::<u64>;
    let mut current_seat = initial.seat();
    let mut next_poll = 0u64;
    let mut ready = false;
    let mut last_status = String::new();
    let mut prepared_density = None;
    loop {
        let Some((viewport, dpi)) = resolve_display_geometry(
            mq::screen_width(),
            mq::screen_height(),
            mq::screen_dpi_scale(),
        ) else {
            mq::next_frame().await;
            continue;
        };
        let density = tabula_render_macroquad::density_for_dpi(dpi);
        if prepared_density != Some(density) {
            if resources
                .prepare(renderer, LocalAssetScene::Gameplay, dpi)
                .await
                .is_err()
            {
                online.disconnect();
                unavailable().await;
                return;
            }
            prepared_density = Some(density);
        }
        let now = presentation_now_ms();
        let frame = renderer.begin_frame(viewport, dpi, now, *theme);
        let board_viewport = tabula_presentation::Viewport::new(glam::Vec2::new(
            viewport.size().x,
            (viewport.size().y - 56.0).max(1.0),
        ))
        .expect("positive online viewport");
        let board_frame = FrameCtx::new(board_viewport, dpi, now, *theme);
        online.local_mut().sync_frame(&board_frame);
        if let Some(future) = inflight.as_mut() {
            if let Poll::Ready(result) = poll_request(future) {
                inflight = None;
                let previously_pending = online.has_pending();
                match result {
                    Ok(bytes) if bytes.len() <= tabula_match_http::MAX_RESPONSE_BYTES => {
                        if let Ok(response) = serde_json::from_slice::<TransportResponse>(&bytes) {
                            match response {
                                TransportResponse::Recovering => {
                                    online.recover();
                                    recover_request = true;
                                    command = None;
                                }
                                TransportResponse::Resync { bootstrap } => {
                                    if bootstrap.game_id != metadata.id().as_str()
                                        || bootstrap.game_version != metadata.version().as_str()
                                    {
                                        online.disconnect();
                                    } else {
                                        host_generation = bootstrap.transport_generation;
                                        current_seat = bootstrap.attachment.seat();
                                        match online.resync(
                                            bootstrap.attachment.operation_scope(),
                                            bootstrap.attachment.next_seq(),
                                            bootstrap.attachment.frames(),
                                        ) {
                                            Ok(retry) => command = retry,
                                            Err(_) => online.disconnect(),
                                        }
                                        if bootstrap.pending_unknown {
                                            online.mark_unknown();
                                            command = None;
                                        }
                                        online.local_mut().set_reduced_motion(reduced_motion);
                                        online.local_mut().sync_frame(&board_frame);
                                        if !online.has_pending()
                                            && online.state() == DirectState::Ready
                                        {
                                            recovery_cycles = 0;
                                        }
                                        // The replacement starts at revision0 with no animation/cue replay.
                                        last_status.clear();
                                    }
                                }
                            }
                        } else if let Ok(response) = serde_json::from_slice::<MatchFrames>(&bytes) {
                            if let Ok(cues) = online.receive(response.frames(), &board_frame) {
                                play_cues(audio, &cues);
                            } else {
                                online.recover();
                                recover_request = true;
                                command = None;
                            }
                        } else {
                            online.disconnect();
                        }
                    }
                    _ => online.disconnect(),
                }
                if online.state() == DirectState::ReadOnly {
                    let _ = mq::load_file("tabula-online-unknown.txt").await;
                } else if previously_pending && !online.has_pending() {
                    let _ = mq::load_file("tabula-online-settled.txt").await;
                    pending_since = None;
                    recovery_cycles = 0;
                }
                next_poll = now.saturating_add(500);
            }
        }
        for event in renderer.drain_input() {
            match online.on_input(&event) {
                Ok(Some(value)) => command = Some(value),
                Ok(None) => {}
                Err(_) => online.disconnect(),
            }
        }
        if inflight.is_none() && online.state() != DirectState::Disconnected {
            if online.state() == DirectState::Sending
                && pending_since.is_some_and(|sent| now.saturating_sub(sent) >= 30000)
            {
                online.recover();
                recover_request = true;
                command = None;
            }
            if recover_request {
                recovery_cycles = recovery_cycles.saturating_add(1);
                if recovery_cycles > 6 {
                    let _ = mq::load_file("tabula-online-unresolved.txt").await;
                    online.disconnect();
                } else {
                    recover_request = false;
                    pending_since = None;
                    inflight = Some(request("tabula-online-recover.txt".into()));
                }
            } else if let Some(command) = command.take() {
                match serde_json::to_vec(&command) {
                    Ok(bytes) if bytes.len() <= tabula_match_http::MAX_REQUEST_BYTES => {
                        pending_since = Some(now);
                        inflight = Some(request(format!("tabula-online-command/{}", hex(&bytes))));
                    }
                    _ => online.disconnect(),
                }
            } else if now >= next_poll {
                inflight = Some(request("tabula-online-poll.txt".into()));
            }
        }
        // Authority loss discards the projection before either board or a11y output.
        // This host operation synchronously conceals pixels and retires the document.
        let Some(scene) = online.present(&board_frame) else {
            if matches!(
                online.state(),
                DirectState::Recovering | DirectState::UnknownResult | DirectState::Resyncing
            ) {
                draw_status(
                    renderer,
                    &frame,
                    "Recovering · moves blocked · the previous move result may be unknown",
                );
                let _ = renderer.end_frame();
                mq::next_frame().await;
                continue;
            }
            unavailable_with_pending(online.has_pending()).await;
            return;
        };
        if renderer.submit(&scene).is_err() {
            online.disconnect();
            unavailable_with_pending(online.has_pending()).await;
            return;
        }
        let message = match online.state() {
            DirectState::Sending => "Sending move…",
            DirectState::Ready => {
                if online.rejection().is_some() {
                    "The server rejected that action. Choose another move"
                } else {
                    "Connected · server-authoritative"
                }
            }
            DirectState::ReadOnly => "Read-only · earlier move result unknown · do not repeat it",
            _ => "Moves are blocked",
        };
        draw_status(renderer, &frame, message);
        let status = serde_json::json!({ "generation": host_generation, "seat": current_seat, "revision": online.revision(), "status": online.description().expect("present projection has a description"), "connection": message }).to_string();
        if renderer.end_frame().is_err() {
            online.disconnect();
            unavailable_with_pending(online.has_pending()).await;
            return;
        }
        mq::next_frame().await;
        if !ready {
            ready = true;
            notify_runtime_ready().await;
        }
        if status != last_status {
            last_status = status.clone();
            // Bounded presenter facts only; the host does not decode a board.
            let _ =
                mq::load_file(&format!("tabula-online-status/{}", hex(status.as_bytes()))).await;
        }
    }
}
// No private status or error body crosses this operation.
async fn unavailable() {
    unavailable_with_pending(false).await;
}
async fn unavailable_with_pending(pending: bool) {
    let _ = mq::load_file(if pending {
        "tabula-online-unresolved.txt"
    } else {
        "tabula-online-unavailable.txt"
    })
    .await;
}
#[allow(clippy::float_arithmetic)] // Screen-space presentation only.
fn draw_status(renderer: &mut MacroquadRenderer, frame: &FrameCtx, message: &str) {
    use tabula_presentation::{Align, Camera2D, Layer, RenderCmd, RenderListBuilder};
    let mut builder = RenderListBuilder::new(Camera2D::default());
    let _ = builder.push(RenderCmd::Text {
        text: message.into(),
        at: glam::Vec2::new(16.0, frame.viewport().size().y - 32.0),
        style: tabula_design::TextStyleToken::BodyMd,
        align: Align::Start,
        max_width: tabula_design::Positive::new((frame.viewport().size().x - 32.0).max(1.0)).ok(),
        color: frame.theme().color.on_surface,
        layer: Layer::HUD,
        z: 0,
    });
    if let Ok(scene) = builder.finish() {
        let _ = renderer.submit(&scene);
    }
}
