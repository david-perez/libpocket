use reqwest::Url;
use thiserror::Error;

const ENDPOINT: &str = "https://getpocket.com/v3";
const REDIRECT_URL: &str = "https://getpocket.com";

type RequestToken = String;
type AuthorizationCode = String;

// TODO Move to utils?
pub fn url(method: &str) -> Url {
    let url = format!("{}{}", ENDPOINT, method);
    Url::parse(&url).unwrap_or_else(|_| panic!("Could not parse URL: {}", url))
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Unexpected OAuth error: `{0}`")]
    OAuthError(#[from] reqwest::Error),

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

async fn request<T: serde::Serialize + ?Sized>(
    client: &reqwest::Client,
    url: Url,
    params: &T,
) -> Result<String, AuthError> {
    let res = client.post(url).form(&params).send().await?;
    let body = res.text().await?;
    Ok(body)
}

pub async fn get_request_token(consumer_key: &str) -> Result<RequestToken, AuthError> {
    let client = reqwest::Client::new(); // TODO

    let body = request(
        &client,
        url("/oauth/request"),
        &[
            ("consumer_key", consumer_key),
            ("redirect_uri", REDIRECT_URL),
        ],
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
    let client = reqwest::Client::new(); // TODO

    let body = request(
        &client,
        url("/oauth/authorize"),
        &[("consumer_key", consumer_key), ("code", &token)],
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
