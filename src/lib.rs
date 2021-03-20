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

#[derive(Serialize, Debug)]
#[serde(rename_all = "lowercase", tag = "action")]
// TODO Turns out that while the docs specify the timestamps have to be strings, sending  numbers
// works fine.
// TODO We can probably make it so that `url` and `item_id` can be references.
// `Add` and `Readd` are the only ones for which the API returns an object akin to an `Item`.
// The rest of the actions return `true`.
pub enum Action {
    Add { url: String, time: u64 }, // TODO More options
    Archive { item_id: ItemId, time: u64 },
    Readd { item_id: ItemId, time: u64 },
    Favorite { item_id: ItemId, time: u64 },
    Unfavorite { item_id: ItemId, time: u64 },
    Delete { item_id: ItemId, time: u64 },
    // TODO the rest.
}

impl Client {
    pub async fn archive<'a, T>(&self, items: T)
    where
        T: IntoIterator<Item = &'a Item>,
    {
        let actions = items.into_iter().map(|item| Action::Archive {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await;
    }

    pub async fn readd<'a, T>(&self, items: T)
    where
        T: IntoIterator<Item = &'a Item>,
    {
        let actions = items.into_iter().map(|item| Action::Readd {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await;
    }

    pub async fn favorite<'a, T>(&self, items: T)
    where
        T: IntoIterator<Item = &'a Item>,
    {
        let actions = items.into_iter().map(|item| Action::Favorite {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await;
    }

    pub async fn unfavorite<'a, T>(&self, items: T)
    where
        T: IntoIterator<Item = &'a Item>,
    {
        let actions = items.into_iter().map(|item| Action::Unfavorite {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await;
    }

    pub async fn add_urls<'a, T>(&self, urls: T)
    where
        T: IntoIterator<Item = &'a str>,
    {
        let actions = urls.into_iter().map(|url| Action::Add {
            url: String::from(url),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await;
    }

    pub async fn delete<'a, T>(&self, items: T)
    where
        T: IntoIterator<Item = &'a Item>,
    {
        let actions = items.into_iter().map(|item| Action::Delete {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await;
    }

    fn auth(&self) -> serde_json::Value {
        json!({
            "consumer_key": &self.consumer_key,
            "access_token": &self.authorization_code,
        })
    }

    pub async fn get(&self, get_input: &GetInput) -> ClientResponse<ReadingList> {
        let method = url("/get");

        let mut payload =
            serde_json::to_value(get_input).expect("Unable to convert input to JSON value");
        payload.merge(self.auth());

        let response = self.request(method, payload.to_string()).await;

        // dbg!(&response);

        let mut reading_list: ReadingList = Default::default();

        match parse_get_response(&response) {
            Ok(ResponseState::NoMore) => (),
            Ok(ResponseState::Parsed(parsed_response)) => {
                reading_list.extend(parsed_response.list.into_iter());
            }
            Err(e) => return Err(ClientError::ParseJSON(e)),
        }

        Ok(reading_list)
    }

    pub async fn list_all(&self) -> ClientResponse<ReadingList> {
        let mut reading_list: ReadingList = Default::default();

        let mut offset = 0;

        loop {
            let method = url("/get");

            let get_input = GetInputBuilder::default()
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

            // dbg!(&response);

            match parse_get_response(&response) {
                Ok(ResponseState::NoMore) => break,
                Ok(ResponseState::Parsed(parsed_response)) => {
                    offset += 1;
                    reading_list.extend(parsed_response.list.into_iter());
                }
                Err(e) => return Err(ClientError::ParseJSON(e)),
            }
        }

        Ok(reading_list)
    }

    pub async fn modify<T>(&self, actions: T)
    where
        T: IntoIterator<Item = Action>,
    {
        let method = url("/send");

        let mut payload = json!({ "actions": actions.into_iter().collect::<Vec<Action>>() });
        payload.merge(self.auth());

        let _response = self.request(method, payload.to_string()).await;

        // TODO Parse response.
        // response = "{\"action_results\":[false],\"action_errors\":[null],\"status\":1}"
        // {"action_results":[true],"status":1}

        // dbg!(&response);
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

        let ret = String::from_utf8(body_bytes.to_vec()).expect("Response was not valid UTF-8");

        // dbg!(&ret);

        ret
    }
}

fn parse_get_response(response: &str) -> Result<ResponseState, serde_json::Error> {
    match serde_json::from_str::<ReadingListResponse>(response) {
        Ok(r) => Ok(ResponseState::Parsed(r)),
        Err(e) => match serde_json::from_str::<EmptyReadingListResponse>(response) {
            Ok(r) => {
                if r.list.is_empty() {
                    // TODO I think the response sets '"status": 2' when there's no more, and list
                    // gets set to an empty array.
                    Ok(ResponseState::NoMore)
                } else {
                    // Received a non-empty array instead of an object for the key "list".
                    Err(e)
                }
            }
            Err(_) => Err(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_empty_list_object() {
        let response = r#"{ "list": {}}"#;
        match parse_get_response(&response) {
            Ok(ResponseState::Parsed(_)) => (),
            _ => panic!("This should have been parsed"),
        }
    }

    #[test]
    fn deserialize_empty_list_array() {
        let response = r#"{ "list": []}"#;
        match parse_get_response(&response) {
            Ok(ResponseState::NoMore) => (),
            _ => panic!("This should signal an empty list"),
        }
    }

    #[test]
    #[should_panic]
    fn deserialize_unparseable_response() {
        let response = r#"{ "list": "#;
        parse_get_response(&response).unwrap();
    }
}
