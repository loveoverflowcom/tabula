//! Browser transport glue around the existing game-generic Macroquad presenter.
//! Fetch futures are polled once per frame, never inside presentation.
use super::{notify_runtime_ready, play_cues, presentation_now_ms, SetViewport};
use macroquad::prelude as mq;
use std::{future::Future, pin::Pin, task::{Context, Poll, Waker}};
use tabula_game_api::{GameModule, GameRules};
use tabula_game_client::{online::OnlineMatch, fixture_assets::{LocalAssetScene, LocalSpriteResources}, resolve_display_geometry};
use tabula_match_http::{MatchAttachment, MatchFrames};
use tabula_net_client::direct::DirectState;
use tabula_presentation::{FrameCtx, GamePresentation, Renderer};
use tabula_render_macroquad::{MacroquadRenderer, MacroquadAudioSink};

type ResponseFuture = Pin<Box<dyn Future<Output = Result<Vec<u8>, macroquad::Error>>>>;
fn request(name: String) -> ResponseFuture { Box::pin(async move { mq::load_file(&name).await }) }
fn poll_request(future: &mut ResponseFuture) -> Poll<Result<Vec<u8>, macroquad::Error>> { future.as_mut().poll(&mut Context::from_waker(Waker::noop())) }
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes { let _ = write!(out, "{byte:02x}"); }
    out
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialResponse { game_id: String, game_version: String, attachment: MatchAttachment }

/// One online loop, holding projections and local presentation only.
#[allow(clippy::too_many_lines, clippy::float_arithmetic)] // Renderer-only geometry, never authoritative.
pub(super) async fn run_online<M, P>(renderer: &mut MacroquadRenderer, audio: &mut MacroquadAudioSink, theme: &tabula_design::Theme, match_id: &str, reduced_motion: bool, resources: &LocalSpriteResources)
where M: GameModule, P: GamePresentation<Rules = M::Rules>, P::Local: SetViewport, <M::Rules as GameRules>::View: serde::de::DeserializeOwned, <M::Rules as GameRules>::ViewEvent: serde::de::DeserializeOwned {
    let initial = match mq::load_file("tabula-online-attach.txt").await {
        Ok(bytes) if bytes.len() <= tabula_match_http::MAX_RESPONSE_BYTES => serde_json::from_slice::<InitialResponse>(&bytes).map_err(|_| "The server attachment is incompatible"),
        _ => Err("The online board could not attach. Return to Tabula and sign in again"),
    };
    let initial = match initial { Ok(value) => value, Err(message) => { super::show_asset_failure(renderer, theme, message).await; return; } };
    let Ok(id) = tabula_match_http::parse_match_id(match_id) else { super::show_asset_failure(renderer, theme, "Invalid online match address").await; return; };
    let metadata = M::metadata();
    if initial.game_id != metadata.id().as_str() || initial.game_version != metadata.version().as_str() { super::show_asset_failure(renderer, theme, "The linked game package does not match this online game").await; return; }
    let initial = initial.attachment;
    let Ok(mut online) = OnlineMatch::<M::Rules, P>::new(id, metadata.id().clone(), metadata.version().clone(), initial.next_seq(), initial.frames()) else { super::show_asset_failure(renderer, theme, "The online projection is incompatible").await; return; };
    online.local_mut().set_reduced_motion(reduced_motion);
    let mut inflight = None::<ResponseFuture>;
    let mut command = None::<tabula_protocol::ClientEnvelope>;
    let mut next_poll = 0u64;
    let mut ready = false;
    let mut last_status = String::new();
    let mut prepared_density = None;
    let mut transport_message = None::<&str>;
    loop {
        let Some((viewport, dpi)) = resolve_display_geometry(mq::screen_width(), mq::screen_height(), mq::screen_dpi_scale()) else { mq::next_frame().await; continue; };
        let density = tabula_render_macroquad::density_for_dpi(dpi);
        if prepared_density != Some(density) {
            if let Err(error) = resources.prepare(renderer, LocalAssetScene::Gameplay, dpi).await { super::show_asset_failure(renderer, theme, &error).await; return; }
            prepared_density = Some(density);
        }
        let now = presentation_now_ms();
        let frame = renderer.begin_frame(viewport, dpi, now, *theme);
        let board_viewport = tabula_presentation::Viewport::new(glam::Vec2::new(viewport.size().x, (viewport.size().y - 56.0).max(1.0))).expect("positive online viewport");
        let board_frame = FrameCtx::new(board_viewport, dpi, now, *theme);
        online.local_mut().sync_frame(&board_frame);
        if let Some(future) = inflight.as_mut() {
            if let Poll::Ready(result) = poll_request(future) {
                inflight = None;
                match result {
                    Ok(bytes) if bytes.len() <= tabula_match_http::MAX_RESPONSE_BYTES => {
                        if let Ok(response) = serde_json::from_slice::<MatchFrames>(&bytes) {
                            match online.receive(response.frames(), &board_frame) {
                                Ok(cues) => play_cues(audio, &cues),
                                Err(message) => { online.disconnect(); transport_message = Some(message); }
                            }
                        } else {
                            online.disconnect();
                            transport_message = Some("Connection lost or incompatible response. Moves are blocked. Return to Tabula to reopen");
                        }
                    }
                    _ => { online.disconnect(); transport_message = Some("Connection lost. Moves are blocked. Return to Tabula to reopen this match"); }
                }
                next_poll = now.saturating_add(500);
            }
        }
        for event in renderer.drain_input() {
            match online.on_input(&event) {
                Ok(Some(value)) => command = Some(value),
                Ok(None) => {}
                Err(message) => { online.disconnect(); transport_message = Some(message); }
            }
        }
        if inflight.is_none() && online.state() != DirectState::Disconnected {
            if let Some(command) = command.take() {
                match serde_json::to_vec(&command) {
                    Ok(bytes) if bytes.len() <= tabula_match_http::MAX_REQUEST_BYTES => { inflight = Some(request(format!("tabula-online-command/{}", hex(&bytes)))); }
                    _ => { online.disconnect(); transport_message = Some("The online command is too large"); }
                }
            } else if now >= next_poll { inflight = Some(request("tabula-online-poll.txt".into())); }
        }
        if renderer.submit(&online.present(&board_frame)).is_err() { online.disconnect(); transport_message = Some("The board could not render"); }
        let message = transport_message.unwrap_or_else(|| match online.state() {
            DirectState::Sending => "Sending move…",
            DirectState::Ready => if online.rejection().is_some() { "The server rejected that action. Choose another move" } else { "Connected · server-authoritative" },
            _ => "Moves are blocked",
        });
        draw_status(renderer, &frame, message);
        let status = serde_json::json!({ "seat": initial.seat(), "revision": online.revision(), "status": online.description(), "connection": message }).to_string();
        let frame_ok = renderer.end_frame().is_ok();
        mq::next_frame().await;
        if frame_ok && !ready { ready = true; notify_runtime_ready().await; }
        if status != last_status {
            last_status = status.clone();
            // Bounded presenter facts only; the host does not decode a board.
            let _ = mq::load_file(&format!("tabula-online-status/{}", hex(status.as_bytes()))).await;
        }
    }
}
#[allow(clippy::float_arithmetic)] // Screen-space presentation only.
fn draw_status(renderer: &mut MacroquadRenderer, frame: &FrameCtx, message: &str) {
    use tabula_presentation::{Align, Camera2D, Layer, RenderCmd, RenderListBuilder};
    let mut builder = RenderListBuilder::new(Camera2D::default());
    let _ = builder.push(RenderCmd::Text { text: message.into(), at: glam::Vec2::new(16.0, frame.viewport().size().y - 32.0), style: tabula_design::TextStyleToken::BodyMd, align: Align::Start, max_width: tabula_design::Positive::new((frame.viewport().size().x - 32.0).max(1.0)).ok(), color: frame.theme().color.on_surface, layer: Layer::HUD, z: 0 });
    if let Ok(scene) = builder.finish() { let _ = renderer.submit(&scene); }
}
