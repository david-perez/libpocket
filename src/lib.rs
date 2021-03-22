use derive_builder::Builder;
use json_value_merge::Merge;
use log::{debug, info};
use reqwest::Url;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

mod auth;
mod model;

pub use auth::*;
pub use model::*;

const DEFAULT_COUNT: u32 = 5000;

#[derive(Debug, Deserialize)]
struct ReadingListResponse {
    list: ReadingList,
}

#[derive(Debug, Deserialize)]
struct EmptyReadingListResponse {
    // Apparently, Pocket changes the "list" value from an object to an empty JSON array when the
    // response contains no items.
    list: Vec<Item>,
}

#[derive(Debug)]
enum ResponseState {
    Parsed(ReadingListResponse),
    NoMore,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ActionError {
    pub code: u16,
    pub message: String,

    #[serde(rename = "type")]
    pub error_type: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct ModifyResponseInner {
    action_errors: Vec<Option<ActionError>>,
    action_results: Vec<ModifiedItemOrBool>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum ModifiedItemOrBool {
    ModifiedItem(ModifiedItem),
    Bool(bool),
}

/// Any fallible operation by the client models its errors using one of this type's variants.
#[derive(Debug, Error)]
pub enum ClientError {
    #[error("error parsing JSON response from Pocket API; response: {0}")]
    ParseJson(#[from] serde_json::Error),

    #[error("error performing request to Pocket API: {0}")]
    HttpError(#[from] reqwest::Error),
}

pub type ModifyResponse = Vec<Result<Option<ModifiedItem>, ActionError>>;

pub type ClientResult<T> = Result<T, ClientError>;
pub type ModifyResult = ClientResult<ModifyResponse>;

#[derive(Debug, Serialize, Clone)]
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

#[derive(Debug, Serialize, Clone)]
pub enum TagFilter {
    /// Only return items tagged with a tag name.
    TagName(String),
    /// Only return untagged items.
    #[serde(rename = "_untagged_")]
    Untagged,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// Only return articles.
    Article,
    /// Only return videos, or articles with embedded videos.
    Video,
    /// Only return images.
    Image,
}

#[derive(Debug, Serialize, Clone)]
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DetailType {
    /// Return basic information about each item, including title, URL, status, and more.
    Simple,
    /// Return all data about each item, including tags, images, authors, videos, and more.
    Complete,
}

#[serde_with::skip_serializing_none]
#[builder(default)]
#[derive(Debug, Serialize, Builder, Default)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug)]
pub struct Client {
    /// Internal member to perform requests to the Pocket API.
    http: reqwest::Client,

    /// Your application's consumer key.
    consumer_key: String,

    /// The specific user's access token code.
    authorization_code: String,
}

impl Client {
    /// Initialize a Pocket API client.
    ///
    /// Parameters:
    /// - consumer_key - your application's consumer key.
    /// - authorization_code - the specific user's access token code
    ///
    /// [Reference](https://getpocket.com/developer/docs/authentication)
    pub fn new(consumer_key: String, authorization_code: String) -> Self {
        info!("Client::new()");
        debug!(
            "consumer_key: {}, authorization_code: {}",
            &consumer_key, &authorization_code
        );
        Client {
            http: reqwest::Client::new(),
            consumer_key,
            authorization_code,
        }
    }

    // TODO Docs
    pub async fn archive<'a, T>(&self, items: T) -> ModifyResult
    where
        T: IntoIterator<Item = &'a Item>,
    {
        info!("Client::archive()");
        let actions = items.into_iter().map(|item| Action::Archive {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await
    }

    pub async fn readd<'a, T>(&self, items: T) -> ModifyResult
    where
        T: IntoIterator<Item = &'a Item>,
    {
        info!("Client::readd()");
        let actions = items.into_iter().map(|item| Action::Readd {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await
    }

    pub async fn favorite<'a, T>(&self, items: T) -> ModifyResult
    where
        T: IntoIterator<Item = &'a Item>,
    {
        info!("Client::favorite()");
        let actions = items.into_iter().map(|item| Action::Favorite {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await
    }

    pub async fn unfavorite<'a, T>(&self, items: T) -> ModifyResult
    where
        T: IntoIterator<Item = &'a Item>,
    {
        info!("Client::unfavorite()");
        let actions = items.into_iter().map(|item| Action::Unfavorite {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await
    }

    pub async fn add_urls<'a, T>(&self, urls: T) -> ModifyResult
    where
        T: IntoIterator<Item = &'a str>,
    {
        info!("Client::add_urls()");
        let actions = urls.into_iter().map(|url| Action::Add {
            url: String::from(url),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await
    }

    pub async fn delete<'a, T>(&self, items: T) -> ModifyResult
    where
        T: IntoIterator<Item = &'a Item>,
    {
        info!("Client::delete()");
        let actions = items.into_iter().map(|item| Action::Delete {
            item_id: item.item_id.clone(),
            time: chrono::Utc::now().timestamp() as u64,
        });

        self.modify(actions).await
    }

    fn auth(&self) -> serde_json::Value {
        json!({
            "consumer_key": &self.consumer_key,
            "access_token": &self.authorization_code,
        })
    }

    pub async fn get(&self, get_input: &GetInput) -> ClientResult<ReadingList> {
        info!("Client::get()");
        debug!("get_input: {:#?}", &get_input);
        let method = url("/get");

        let payload =
            serde_json::to_value(get_input).expect("Unable to convert input to JSON value");

        let response_body = self.post_json(method, payload).await?;

        let mut reading_list: ReadingList = Default::default();

        match parse_get_response_body(&response_body) {
            Ok(ResponseState::NoMore) => (),
            Ok(ResponseState::Parsed(parsed_response)) => {
                reading_list.extend(parsed_response.list.into_iter());
            }
            Err(e) => return Err(ClientError::ParseJson(e)),
        }

        Ok(reading_list)
    }

    pub async fn list_all(&self) -> ClientResult<ReadingList> {
        info!("Client::list_all()");
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
            debug!("get_input: {:#?}", &get_input);

            let payload =
                serde_json::to_value(get_input).expect("Unable to convert input to JSON value");

            let response_body = self.post_json(method, payload).await?;

            match parse_get_response_body(&response_body) {
                Ok(ResponseState::NoMore) => break,
                Ok(ResponseState::Parsed(parsed_response)) => {
                    offset += 1;
                    reading_list.extend(parsed_response.list.into_iter());
                }
                Err(e) => return Err(ClientError::ParseJson(e)),
            }
        }

        Ok(reading_list)
    }

    pub async fn modify<T>(&self, actions: T) -> ModifyResult
    where
        T: IntoIterator<Item = Action>,
    {
        info!("Client::modify()");
        let method = url("/send");
        let actions = actions.into_iter().collect::<Vec<Action>>();
        debug!("actions: {:#?}", &actions);
        let payload = json!({ "actions": actions });
        let response_body = self.post_json(method, payload).await?;

        let parsed = parse_send_response_body(&response_body)?;

        let ret = parsed
            .action_results
            .into_iter()
            .zip(parsed.action_errors)
            .map(|(action_result, action_error)| match action_error {
                Some(action_error) => match action_result {
                    ModifiedItemOrBool::ModifiedItem(modified_item) => {
                        panic!(
                            "Received an error yet the item was modified.
`action_error` = `{:#?}`
`modified_item` = `{:#?}`",
                            action_error, modified_item
                        );
                    }
                    ModifiedItemOrBool::Bool(success) => {
                        if success {
                            panic!(
                                "Received an error yet action_result is true.
`action_error` = `{:#?}`",
                                action_error
                            );
                        }

                        Err(action_error)
                    }
                },
                None => match action_result {
                    ModifiedItemOrBool::ModifiedItem(modified_item) => Ok(Some(modified_item)),
                    ModifiedItemOrBool::Bool(_) => Ok(None),
                },
            })
            .collect();

        // TODO Should I return an iterator?
        Ok(ret)
    }

    async fn post_json(&self, url: Url, mut json: serde_json::Value) -> ClientResult<String> {
        json.merge(self.auth());
        let req = self.http.post(url).json(&json);
        debug!("Request: {:#?}", &req);
        debug!("Request JSON body: {}", &json.to_string());
        let res = req.send().await?;

        // Bubble up non 2XX responses as errors.
        let res = res.error_for_status()?;
        debug!("{:?}", &res);
        let body = res.text().await?;
        debug!("Response body: {:?}", &body);

        Ok(body)
    }
}

fn parse_get_response_body(response: &str) -> Result<ResponseState, serde_json::Error> {
    match serde_json::from_str::<ReadingListResponse>(response) {
        Ok(r) => {
            debug!("Parsed response body: {:#?}", &r);
            Ok(ResponseState::Parsed(r))
        }
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

fn parse_send_response_body(response: &str) -> Result<ModifyResponseInner, serde_json::Error> {
    let ret: ModifyResponseInner = serde_json::from_str(response)?;
    debug!("Parsed response body: {:#?}", &ret);
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_get_empty_list_object() {
        let response = r#"{ "list": {}}"#;
        match parse_get_response_body(&response) {
            Ok(ResponseState::Parsed(_)) => (),
            _ => panic!("This should have been parsed"),
        }
    }

    #[test]
    fn deserialize_get_empty_list_array() {
        let response = r#"{ "list": []}"#;
        match parse_get_response_body(&response) {
            Ok(ResponseState::NoMore) => (),
            _ => panic!("This should signal an empty list"),
        }
    }

    #[test]
    #[should_panic]
    fn deserialize_get_unparseable_response() {
        let response = r#"{ "list": "#;
        parse_get_response_body(&response).unwrap();
    }

    #[test]
    fn deserialize_send_response() {
        let response = r#"{ "action_errors": [null], "action_results": [true]}"#;
        assert_eq!(
            parse_send_response_body(&response).unwrap(),
            ModifyResponseInner {
                action_errors: vec![None],
                action_results: vec![ModifiedItemOrBool::Bool(true)]
            }
        );
    }

    #[test]
    fn deserialize_send_response_with_errors() {
        let response = r#"
{
    "action_errors": [
        {
            "code": 422,
            "message": "Invalid/non-existent URL",
            "type": "Unprocessable Entity"
        }
    ],
    "action_results": [
        false
    ],
    "status": 1
}"#;
        assert_eq!(
            parse_send_response_body(&response).unwrap(),
            ModifyResponseInner {
                action_errors: vec![Some(ActionError {
                    code: 422,
                    message: String::from("Invalid/non-existent URL"),
                    error_type: String::from("Unprocessable Entity"),
                })],
                action_results: vec![ModifiedItemOrBool::Bool(false)]
            }
        );
    }

    #[test]
    fn deserialize_send_unparseable_response() {
        let response = r#"{ "action_errors": [null] }"#;
        parse_send_response_body(&response).unwrap_err();
    }
}
