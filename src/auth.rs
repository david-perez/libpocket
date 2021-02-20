use hyper::{body, header, Body, Method, Request, Uri};
use hyper_rustls::HttpsConnector;
use thiserror::Error;

const ENDPOINT: &str = "https://getpocket.com/v3";
const REDIRECT_URL: &str = "https://getpocket.com";

pub type RequestToken = String;
pub type AuthorizationCode = String;

pub struct Client {
    pub consumer_key: String,
    pub authorization_code: String,
}

pub fn https_client() -> hyper::Client<HttpsConnector<hyper::client::HttpConnector>> {
    let https = HttpsConnector::with_native_roots();
    hyper::Client::builder().build::<_, hyper::Body>(https)
}

// TODO Move to utils?
pub fn url(method: &str) -> Uri {
    let url = format!("{}{}", ENDPOINT, method);
    url.parse()
        .unwrap_or_else(|_| panic!("Could not parse URL: {}", url))
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Unexpected OAuth error: `{0}`")]
    OAuthError(String),

    #[error("Unexpected error while requesting OAuth token: `{0}`")]
    RequestTokenError(String),

    #[error("Unexpected error while requesting authorization code: `{0}`")]
    RequestAuthorizationCode(String),
}

pub fn authorization_url(token: &RequestToken) -> String {
    format!(
        "https://getpocket.com/auth/authorize?request_token={}&redirect_uri={}",
        token, REDIRECT_URL
    )
}

async fn request(
    client: &hyper::Client<HttpsConnector<hyper::client::HttpConnector>>,
    uri: &Uri,
    body: String,
) -> Result<String, AuthError> {
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::CONNECTION, "close")
        .body(Body::from(body))
        .map_err(|_| AuthError::OAuthError(String::from("could not construct request.")))?;

    let response = client
        .request(request)
        .await
        .map_err(|_| AuthError::OAuthError(String::from("could not send request.")))?;

    let body_bytes = body::to_bytes(response.into_body())
        .await
        .map_err(|_| AuthError::OAuthError(String::from("unable to read response body.")))?;

    let body = String::from_utf8(body_bytes.to_vec())
        .map_err(|_| AuthError::OAuthError(String::from("response was not valid UTF-8.")));

    body
}

pub async fn get_request_token(consumer_key: &str) -> Result<RequestToken, AuthError> {
    let client = https_client();

    let body = request(
        &client,
        &url("/oauth/request"),
        format!(
            "consumer_key={}&redirect_uri={}",
            consumer_key, REDIRECT_URL
        ),
    )
    .await?;

    let token = body.split('=').nth(1).ok_or_else(|| {
        AuthError::RequestTokenError(format!(
            r#"could not retrieve token from response body. Body was: "{}""#,
            &body
        ))
    })?;

    Ok(String::from(token))
}

pub async fn get_authorization_code(
    consumer_key: &str,
    token: String,
) -> Result<AuthorizationCode, AuthError> {
    let client = https_client();

    let body = request(
        &client,
        &url("/oauth/authorize"),
        format!("consumer_key={}&code={}", consumer_key, token),
    )
    .await?;

    let first_value = body.split('=').nth(1).ok_or_else(|| {
        AuthError::RequestAuthorizationCode(format!(
            r#"unable to parse response. Response was "{}""#,
            &body
        ))
    })?;

    let code = first_value.split('&').next().ok_or_else(|| {
        AuthError::RequestAuthorizationCode(format!(
            r#"unable to parse response. Response was "{}""#,
            &body
        ))
    })?;

    Ok(String::from(code))
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[tokio::test]
    async fn request_token() {
        let consumer_key = env::var("POCKET_CONSUMER_KEY").expect("POCKET_CONSUMER_KEY not set");

        get_request_token(&consumer_key).await.unwrap();
    }

    #[tokio::test]
    async fn request_token_should_fail_with_invalid_consumer_key() {
        get_request_token("invalid_consumer_key").await.unwrap_err();
    }
}
