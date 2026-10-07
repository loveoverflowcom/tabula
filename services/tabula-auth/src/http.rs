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

/// Explicit enrollment/profile composition; no production entrypoint is opened.
#[cfg(feature = "accounts")]
pub async fn isolated_account_router<A>(
    config: KanidmConfig,
    authority: A,
) -> Result<Router, SessionError>
where
    A: HttpSessionAuthority
        + tabula_session::EnrollmentAuthority
        + tabula_session::AccountProfileAuthority
        + Clone
        + 'static,
    A::ProfilePublication: 'static,
{
    let origin = config.browser_origin().to_owned();
    let provider = KanidmOidc::discover_enrollment(config, authority.clone()).await?;
    let http = IsolatedSessionHttp::new(authority, &origin)?
        .with_account_capabilities(true, false)?
        .with_account_readiness()?;
    Ok(http.router_with_accounts(provider))
}

/// Complete opt-in account and social router; production bootstraps stay closed.
#[cfg(feature = "accounts-social")]
pub async fn isolated_accounts_router<A>(
    config: KanidmConfig,
    authority: A,
) -> Result<Router, SessionError>
where
    A: HttpSessionAuthority
        + tabula_session::EnrollmentAuthority
        + tabula_session::AccountProfileAuthority
        + tabula_lobby::social::SocialAuthority
        + Clone
        + 'static,
    A::ProfilePublication: 'static,
    A::SocialPublication: 'static,
{
    let origin = config.browser_origin().to_owned();
    let provider = KanidmOidc::discover_enrollment(config, authority.clone()).await?;
    let http = IsolatedSessionHttp::new(authority, &origin)?
        .with_account_capabilities(true, true)?
        .with_account_readiness()?;
    let social = http.social_router();
    Ok(http.router_with_accounts(provider).merge(social))
}
