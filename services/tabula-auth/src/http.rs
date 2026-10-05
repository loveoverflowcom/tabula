//! Explicit isolated composition. Default service bootstrap remains closed.
use crate::{config::KanidmConfig, oidc::KanidmOidc};
use axum::Router;
use tabula_session::{HttpSessionAuthority, SessionError};
use tabula_session_http::isolated::IsolatedSessionHttp;

/// Real configured provider plus existing current-authority HTTP routes. This
/// creates no listener, provider account/client, migration or production grant.
pub async fn isolated_router<A: HttpSessionAuthority + Clone + 'static>(
    config: KanidmConfig,
    authority: A,
) -> Result<Router, SessionError> {
    let origin = config.browser_origin().to_owned();
    let provider = KanidmOidc::discover(config, authority.clone()).await?;
    Ok(IsolatedSessionHttp::new(authority, &origin)?.router_with_login(provider))
}
