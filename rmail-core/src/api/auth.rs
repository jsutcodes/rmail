//! OAuth 2.0 (Authorization Code + PKCE) login against the Microsoft
//! identity platform, so `rmail` can talk to Outlook/365 via Microsoft
//! Graph.
//!
//! Flow:
//! 1. Build the Microsoft `/authorize` URL with a PKCE challenge + CSRF
//!    state, and open it in the user's default browser.
//! 2. Spin up a tiny local HTTP server on `127.0.0.1:<redirect_port>` to
//!    catch the single redirect Microsoft sends back with `code`+`state`.
//! 3. Exchange the code for an access/refresh token pair.
//! 4. Persist the tokens in the OS keychain (via the `keyring` crate) so
//!    the user doesn't have to log in every launch.
//!
//! Requires an Azure AD "public client" app registration - see the
//! project README for the exact steps (redirect URI, API permissions).

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use oauth2::basic::BasicClient;
use oauth2::{
    reqwest as oauth2_reqwest, AuthUrl, AuthorizationCode, ClientId, CsrfToken, EndpointNotSet,
    EndpointSet, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};

/// Default port for the loopback redirect listener. Must match the
/// redirect URI registered on the Azure AD app
/// (`http://localhost:8733/callback`).
pub const DEFAULT_REDIRECT_PORT: u16 = 8733;

/// Keyring service name under which tokens are stored.
const KEYRING_SERVICE: &str = "rmail-outlook";

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(
        "missing RMAIL_CLIENT_ID environment variable; register an Azure AD \
         public client app and set RMAIL_CLIENT_ID (see README)"
    )]
    MissingClientId,
    #[error("timed out waiting for the browser redirect; login was aborted")]
    Timeout,
    #[error("state returned by Microsoft did not match the request; aborting for safety")]
    StateMismatch,
    #[error("redirect did not include an authorization code")]
    MissingCode,
    #[error("no stored credentials for account `{0}`; run login first")]
    NotLoggedIn(String),
}

/// A fully negotiated access/refresh token pair for one account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
}

impl TokenSet {
    /// True if the token is expired or will expire within the next minute.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at - chrono::Duration::seconds(60)
    }
}

/// Configuration for the OAuth login flow. Populated from environment
/// variables so no secrets live in source control.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub tenant_id: String,
    pub redirect_port: u16,
    pub scopes: Vec<String>,
}

impl AuthConfig {
    /// Reads `RMAIL_CLIENT_ID` (required), `RMAIL_TENANT_ID` (default
    /// `"common"`), and `RMAIL_REDIRECT_PORT` (default
    /// [`DEFAULT_REDIRECT_PORT`]) from the environment.
    pub fn from_env() -> Result<Self> {
        let client_id = std::env::var("RMAIL_CLIENT_ID").map_err(|_| AuthError::MissingClientId)?;
        let tenant_id = std::env::var("RMAIL_TENANT_ID").unwrap_or_else(|_| "common".to_string());
        let redirect_port = std::env::var("RMAIL_REDIRECT_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_REDIRECT_PORT);
        Ok(Self {
            client_id,
            tenant_id,
            redirect_port,
            scopes: default_scopes(),
        })
    }
}

fn default_scopes() -> Vec<String> {
    [
        "offline_access",
        "User.Read",
        "Mail.Read",
        "Mail.ReadWrite",
        "Mail.Send",
        "Calendars.Read",
        "MailboxSettings.Read",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// The concrete `oauth2` client type once auth/token/redirect URLs are set.
type MsClient = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

fn build_client(config: &AuthConfig) -> Result<MsClient> {
    let auth_url = AuthUrl::new(format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
        config.tenant_id
    ))?;
    let token_url = TokenUrl::new(format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        config.tenant_id
    ))?;
    let redirect_url = RedirectUrl::new(format!(
        "http://localhost:{}/callback",
        config.redirect_port
    ))?;

    Ok(BasicClient::new(ClientId::new(config.client_id.clone()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_url))
}

fn http_client() -> Result<oauth2_reqwest::blocking::Client> {
    oauth2_reqwest::blocking::ClientBuilder::new()
        // Following redirects on the token endpoint opens the client up to SSRF.
        .redirect(oauth2_reqwest::redirect::Policy::none())
        .build()
        .context("failed to build HTTP client for token exchange")
}

/// Runs the interactive Authorization Code + PKCE flow: opens the user's
/// browser, waits for the redirect on `127.0.0.1:<redirect_port>`, and
/// exchanges the resulting code for tokens. Blocks the calling thread
/// until login completes or times out (5 minutes); call this from a
/// background thread in a UI application.
pub fn authorize_interactive(config: &AuthConfig) -> Result<TokenSet> {
    let client = build_client(config)?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut auth_request = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_challenge);
    for scope in &config.scopes {
        auth_request = auth_request.add_scope(Scope::new(scope.clone()));
    }
    let (auth_url, csrf_token) = auth_request.url();

    // Bind the listener before opening the browser to avoid a race with
    // the redirect arriving before we're ready.
    let server = tiny_http::Server::http(("127.0.0.1", config.redirect_port)).map_err(|e| {
        anyhow!(
            "failed to bind local redirect listener on 127.0.0.1:{}: {e}",
            config.redirect_port
        )
    })?;

    if webbrowser::open(auth_url.as_str()).is_err() {
        println!("Open this URL in your browser to sign in to Outlook:\n{auth_url}");
    }

    let request = server
        .recv_timeout(Duration::from_secs(300))
        .context("local redirect listener failed")?
        .ok_or(AuthError::Timeout)?;

    let full_url = format!("http://localhost{}", request.url());
    let parsed = url::Url::parse(&full_url)?;
    let params: HashMap<_, _> = parsed.query_pairs().into_owned().collect();

    let body = "<html><body style=\"font-family: sans-serif;\">\
        <h2>Signed in to RMail</h2>\
        <p>You can close this tab and return to the terminal.</p>\
        </body></html>";
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
        .expect("static header is valid");
    let response = tiny_http::Response::from_string(body).with_header(header);
    let _ = request.respond(response);

    let returned_state = params.get("state").ok_or(AuthError::MissingCode)?;
    if returned_state.as_str() != csrf_token.secret().as_str() {
        return Err(AuthError::StateMismatch.into());
    }
    let code = params.get("code").ok_or(AuthError::MissingCode)?.clone();

    let http_client = http_client()?;
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request(&http_client)
        .map_err(|e| anyhow!("token exchange failed: {e}"))?;

    Ok(token_response_to_set(&token_result))
}

/// Exchanges a refresh token for a fresh access/refresh token pair.
pub fn refresh(config: &AuthConfig, refresh_token: &str) -> Result<TokenSet> {
    let client = build_client(config)?;
    let http_client = http_client()?;

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request(&http_client)
        .map_err(|e| anyhow!("token refresh failed: {e}"))?;

    let mut token_set = token_response_to_set(&token_result);
    // Microsoft doesn't always rotate the refresh token; keep the old one
    // if a new one wasn't issued.
    if token_set.refresh_token.is_none() {
        token_set.refresh_token = Some(refresh_token.to_string());
    }
    Ok(token_set)
}

fn token_response_to_set(
    token_result: &oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
) -> TokenSet {
    let expires_in = token_result
        .expires_in()
        .map(|d| d.as_secs() as i64)
        .unwrap_or(3600);
    TokenSet {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
        expires_at: Utc::now() + chrono::Duration::seconds(expires_in),
    }
}

/// Persists tokens for `account` (e.g. the user's email address) in the
/// OS keychain.
pub fn save_tokens(account: &str, tokens: &TokenSet) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, account)?;
    let json = serde_json::to_string(tokens)?;
    entry.set_password(&json)?;
    Ok(())
}

/// Loads previously saved tokens for `account`, if any.
pub fn load_tokens(account: &str) -> Result<Option<TokenSet>> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, account)?;
    match entry.get_password() {
        Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Removes stored tokens for `account` (sign out).
pub fn delete_tokens(account: &str) -> Result<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, account)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Returns a valid (non-expired) token for `account`, transparently
/// refreshing and re-persisting it if it has expired. Errors with
/// [`AuthError::NotLoggedIn`] if there are no stored credentials.
pub fn ensure_valid_token(config: &AuthConfig, account: &str) -> Result<TokenSet> {
    let tokens = load_tokens(account)?
        .ok_or_else(|| AuthError::NotLoggedIn(account.to_string()))?;
    if !tokens.is_expired() {
        return Ok(tokens);
    }
    let refresh_token = tokens
        .refresh_token
        .as_deref()
        .ok_or_else(|| AuthError::NotLoggedIn(account.to_string()))?;
    let refreshed = refresh(config, refresh_token)?;
    save_tokens(account, &refreshed)?;
    Ok(refreshed)
}
