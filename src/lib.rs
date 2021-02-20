use derive_builder::Builder;
use hyper::{body, header, Body, Method, Request, Uri};
use json_value_merge::Merge;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

mod auth;
mod model;

pub use auth::*;
pub use model::*;

const DEFAULT_COUNT: u32 = 5000;

#[derive(Deserialize)]
struct ReadingListResponse {
    list: ReadingList,
}

#[derive(Deserialize)]
struct EmptyReadingListResponse {
    // Apparently, Pocket changes the "list" value from an object to an empty JSON array when the
    // response contains no items.
    list: Vec<Item>,
}

enum ResponseState {
    Parsed(ReadingListResponse),
    NoMore,
    Error(serde_json::Error),
}

enum Action {
    Archive,
    Favorite,
    Add,
}

/// Any fallible operation by the client models its errors using one of this type's variants.
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("error parsing JSON response from Pocket API; response: {0}")]
    ParseJSON(#[from] serde_json::Error),
}

type ClientResponse<T> = std::result::Result<T, ClientError>;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Only return unread items (default).
    Unread,
    /// Only return archived items.
    Archive,
    /// Return both unread and archived items.
    All,
}

impl Default for State {
    fn default() -> Self {
        State::Unread
    }
}

#[derive(Serialize, Clone, Debug)]
pub enum TagFilter {
    /// Only return items tagged with a tag name.
    TagName(String),
    /// Only return untagged items.
    #[serde(rename = "_untagged_")]
    Untagged,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// Only return articles.
    Article,
    /// Only return videos, or articles with embedded videos.
    Video,
    /// Only return images.
    Image,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    /// Return items in order of newest to oldest.
    Newest,
    /// Return items in order of oldest to newest.
    Oldest,
    /// Return items in order of title alphabetically.
    Title,
    /// Return items in order of URL alphabetically.
    Site,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DetailType {
    /// Return basic information about each item, including title, URL, status, and more.
    Simple,
    /// Return all data about each item, including tags, images, authors, videos, and more.
    Complete,
}

#[serde_with::skip_serializing_none]
#[builder(default)]
#[derive(Serialize, Builder, Default)]
pub struct GetInput {
    /// Filter by unread or archived items.
    state: Option<State>,

    /// Filter by item favorite status.
    favorite: Option<FavoriteStatus>,

    /// Filter by item tag.
    tag: Option<TagFilter>,

    /// Filter by item content type (article, video, image).
    content_type: Option<ContentType>,

    /// Sort by newest, oldest, title, or site.
    sort: Option<Sort>,

    /// Return basic information or all information about an item.
    #[serde(rename = "detailType")]
    detail_type: Option<DetailType>,

    /// Only return items whose title or url contain the search string.
    search: Option<String>,

    /// Only return items from a particular domain.
    domain: Option<String>,

    /// Only return items modified since the given since UNIX timestamp.
    since: Option<u64>,

    /// Only return count number of items.
    count: Option<u32>,

    /// Used only with count; start returning from offset position of results.
    offset: Option<u32>,
}

impl Client {
    pub async fn mark_as_read<'a, T>(&self, ids: T)
    where
        T: IntoIterator<Item = &'a str>,
    {
        self.modify(Action::Archive, ids).await;
    }

    pub async fn mark_as_favorite<'a, T>(&self, ids: T)
    where
        T: IntoIterator<Item = &'a str>,
    {
        self.modify(Action::Favorite, ids).await;
    }

    pub async fn add_urls<'a, T>(&self, urls: T)
    where
        T: IntoIterator<Item = &'a str>,
    {
        self.modify(Action::Add, urls).await;
    }

    fn auth(&self) -> serde_json::Value {
        json!({
            "consumer_key": &self.consumer_key,
            "access_token": &self.authorization_code,
        })
    }

    pub async fn get(&self, get_input: &GetInput) -> ClientResponse<ReadingList> {
        let mut reading_list: ReadingList = Default::default();

        let method = url("/get");

        let mut payload =
            serde_json::to_value(get_input).expect("Unable to convert input to JSON value");

        payload.merge(self.auth());

        let response = self.request(method, payload.to_string()).await;

        dbg!(&response);

        match parse_get_response(&response) {
            ResponseState::NoMore => (),
            ResponseState::Parsed(parsed_response) => {
                reading_list.extend(parsed_response.list.into_iter());
            }
            ResponseState::Error(e) => return Err(ClientError::ParseJSON(e)),
        }

        Ok(reading_list)
    }

    pub async fn list_all(&self) -> ClientResponse<ReadingList> {
        let mut reading_list: ReadingList = Default::default();

        let mut offset = 0;

        loop {
            let method = url("/get");

            let get_input = GetInputBuilder::default()
                .sort(Some(Sort::Site))
                .state(Some(State::All))
                .detail_type(Some(DetailType::Complete))
                .count(Some(DEFAULT_COUNT))
                .offset(Some(offset * DEFAULT_COUNT))
                .build()
                .unwrap();

            let mut payload =
                serde_json::to_value(get_input).expect("Unable to convert input to JSON value");

            payload.merge(self.auth());

            let response = self.request(method, payload.to_string()).await;

            dbg!(&response);

            match parse_get_response(&response) {
                ResponseState::NoMore => break,
                ResponseState::Parsed(parsed_response) => {
                    offset += 1;
                    reading_list.extend(parsed_response.list.into_iter());
                }
                ResponseState::Error(e) => return Err(ClientError::ParseJSON(e)),
            }
        }

        Ok(reading_list)
    }

    async fn modify<'a, T>(&self, action: Action, ids: T)
    where
        T: IntoIterator<Item = &'a str>,
    {
        let method = url("/send");
        let action_verb = match action {
            Action::Favorite => "favorite",
            Action::Archive => "archive",
            Action::Add => "add",
        };
        let item_key = match action {
            Action::Add => "url",
            _ => "item_id",
        };
        let time = chrono::Utc::now().timestamp();
        let actions: Vec<String> = ids
            .into_iter()
            .map(|id| {
                format!(
                    r##"{{ "action": "{}", "{}": "{}", "time": "{}" }}"##,
                    action_verb, item_key, id, time
                )
            })
            .collect();
        let payload = format!(
            r##"{{ "consumer_key":"{}",
                               "access_token":"{}",
                               "actions": [{}]
                               }}"##,
            &self.consumer_key,
            &self.authorization_code,
            actions.join(", ")
        );

        self.request(method, payload).await;
    }

    async fn request(&self, uri: Uri, payload: String) -> String {
        let client = auth::https_client();

        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONNECTION, "close")
            .body(Body::from(payload.clone()))
            .unwrap();

        let res = client
            .request(req)
            .await
            .unwrap_or_else(|_| panic!("Could not make request with payload: {}", &payload));

        let body_bytes = body::to_bytes(res.into_body())
            .await
            .expect("Could not read the HTTP request's body");

        String::from_utf8(body_bytes.to_vec()).expect("Response was not valid UTF-8")
    }
}

fn parse_get_response(response: &str) -> ResponseState {
    match serde_json::from_str::<ReadingListResponse>(response) {
        Ok(r) => ResponseState::Parsed(r),
        Err(e) => match serde_json::from_str::<EmptyReadingListResponse>(response) {
            Ok(r) => {
                if r.list.is_empty() {
                    // TODO I think the response sets '"status": 2' when there's no more, and list
                    // gets set to an empty array.
                    ResponseState::NoMore
                } else {
                    // Received a non-empty array instead of an object for the key "list".
                    ResponseState::Error(e)
                }
            }
            Err(_) => ResponseState::Error(e),
        },
    }
}

fn fixup_blogspot(url: &str) -> String {
    let split: Vec<_> = url.split(".blogspot.").collect();
    if split.len() == 2 {
        format!("{}.blogspot.com", split[0])
    } else {
        url.into()
    }
}

fn start_domain_from(url: &str) -> usize {
    if url.starts_with("www.") {
        4
    } else {
        0
    }
}

fn cleanup_path(path: &str) -> &str {
    path.trim_end_matches("index.html")
        .trim_end_matches("index.php")
        .trim_end_matches('/')
}

pub fn cleanup_url(url: &str) -> String {
    if let Ok(parsed) = url.parse::<Uri>() {
        let current_host = parsed.host().expect("Cleaned up an url without a host");
        let starts_from = start_domain_from(current_host);

        format!(
            "https://{}{}",
            fixup_blogspot(&current_host[starts_from..]),
            cleanup_path(parsed.path())
        )
    } else {
        url.into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_clean_url_hash() {
        let url_ = "http://example.com#asdfas.fsa";
        assert_eq!(cleanup_url(url_), "https://example.com");
    }

    #[test]
    fn test_clean_url_query() {
        let url_ = "http://example.com?";
        assert_eq!(cleanup_url(url_), "https://example.com");
    }

    #[test]
    fn test_clean_url_keep_same_url() {
        let url_ = "http://another.example.com";
        assert_eq!(cleanup_url(url_), "https://another.example.com");
    }

    #[test]
    fn test_clean_url_keep_https() {
        let url = "https://another.example.com";
        assert_eq!(cleanup_url(url), "https://another.example.com");
    }

    #[test]
    fn test_cleanup_blogspot_first_tld() {
        let url = "https://this-is-a.blogspot.cl/asdf/asdf/asdf?asdf=1";
        assert_eq!(
            cleanup_url(url),
            "https://this-is-a.blogspot.com/asdf/asdf/asdf"
        );
    }

    #[test]
    fn test_cleanup_blogspot_second_tld() {
        let url = "https://this-is-a.blogspot.com.br/asdf/asdf/asdf?asdf=1";
        assert_eq!(
            cleanup_url(url),
            "https://this-is-a.blogspot.com/asdf/asdf/asdf"
        );
    }

    #[test]
    fn test_cleanup_www() {
        let url = "https://www.this-is-a.blogspot.com.br/asdf/asdf/asdf?asdf=1";
        assert_eq!(
            cleanup_url(url),
            "https://this-is-a.blogspot.com/asdf/asdf/asdf"
        );
    }

    #[test]
    fn test_cleanup_https_redirection() {
        let url = "http://www.this-is-a.blogspot.com.br/asdf/asdf/asdf?asdf=2";
        assert_eq!(
            cleanup_url(url),
            "https://this-is-a.blogspot.com/asdf/asdf/asdf"
        );
    }

    #[test]
    fn test_cleanup_urls_are_the_same() {
        let url1 = cleanup_url("https://example.com/hello");
        let url2 = cleanup_url("https://example.com/hello/");
        assert_eq!(url1, url2);
    }

    #[test]
    fn test_cleanup_urls_without_index() {
        let url = "https://example.com/index.php";
        assert_eq!(cleanup_url(url), "https://example.com");
    }

    #[test]
    fn test_cleanup_urls_without_index_html() {
        let url = "https://example.com/index.html";
        assert_eq!(cleanup_url(url), cleanup_url("https://example.com/"));
    }

    #[test]
    fn test_dot_on_files() {
        assert_eq!(
            cleanup_url("https://jenkins.io/2.0/index.html"),
            cleanup_url("https://jenkins.io/2.0/")
        );
    }
}

#[test]
fn test_decoding_empty_object_list() {
    let response = r#"{ "list": {}}"#;
    match parse_get_response(&response) {
        ResponseState::Parsed(_) => (),
        _ => panic!("This should have been parsed"),
    }
}

#[test]
fn test_decoding_empty_pocket_list() {
    let response = r#"{ "list": []}"#;
    match parse_get_response(&response) {
        ResponseState::NoMore => (),
        _ => panic!("This should signal an empty list"),
    }
}

#[test]
fn test_decoding_error() {
    let response = r#"{ "list": "#;
    match parse_get_response(&response) {
        ResponseState::Error(_) => (),
        _ => panic!("This should fail to parse"),
    }
}
