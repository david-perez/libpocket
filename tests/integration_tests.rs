//! These integration tests require a Pocket account's credentials in order to be run, since they
//! hit Pocket's API.
//!
//! The tests rely on shared state (the account's reading list) that is *modified*
//! and asserted on. However, each of them does so on disjoint parts of the state, so the tests can
//! still be run in parallel.

use serde::de::DeserializeOwned;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use thiserror::Error;

use libpocket::{
    ActionError, Client, DetailType, FavoriteStatus, GetInputBuilder, Item, ItemOrDeletedItem,
    ModifiedItem, ModifyResponse, ReadingList, State, Status, Tag,
};

fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[tokio::test]
async fn list_all() {
    init();

    let res = client().list_all().await.unwrap();

    assert!(!res.is_empty());

    // This test requires that the test account contain these 3 items in its reading list.
    let items = vec!["pdf.json", "blog.json", "video.json"]
        .into_iter()
        .map(|filename| deserialize_resource(filename).unwrap());

    assert_contains_items(&res, items);
}

#[tokio::test]
async fn add_and_delete() {
    init();

    let client = client();

    // In the future we may add the Git SHA (grabbing it from an env var or using e.g.
    // https://crates.io/crates/last-git-commit)
    let time_base_64 = base64::encode(now().to_string());
    let url = format!("https://httpbin.org/base64/{}", time_base_64);

    let res_add = client.add_urls(vec![url.as_str()]).await.unwrap();

    let res = client.list_all().await.unwrap();
    assert_contains_given_url_once(&res, &url);

    let item = res.find_given_url(&url).unwrap();
    assert_one_modified_item(&res_add, &item);
    assert_within_5_seconds_of_now(item.time_added);
    // Pocket has a bug (?) whereby `time_updated` is sometimes set to 1 second after `time_X`,
    // where `X` is the action that has just been performed. It seems like their backend is not
    // updating these two fields atomically. In this and the rest of the tests, we therefore check
    // that `time_updated` is not exactly equal to what we expect, but within a 2 second range.
    assert_within_2_seconds(item.time_updated, item.time_added);

    let res = client.delete(vec![item]).await.unwrap();
    assert_one_not_modified_item(&res);
    let res = client.list_all().await.unwrap();
    assert_does_not_contain_given_url(&res, &url);
}

#[tokio::test]
async fn add_invalid_url() {
    init();

    let res = client().add_urls(vec!["savemysoul"]).await.unwrap();
    assert_eq!(res.len(), 1);
    let action_error = res.get(0).unwrap().as_ref().unwrap_err();
    assert_eq!(
        action_error,
        &ActionError {
            code: 422,
            message: String::from("Invalid/non-existent URL"),
            error_type: String::from("Unprocessable Entity")
        }
    );
}

#[tokio::test]
async fn archive_and_readd() {
    init();

    let client = client();
    // This test requires that this URL be already added to the reading list.
    let url = "https://getpocket.com/developer/docs/v3/modify#action_archive";

    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_unread(&item);

    let res = client.archive(vec![&item]).await.unwrap();
    assert_one_not_modified_item(&res);

    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_eq!(item.status, Status::Read);
    assert_within_5_seconds_of_now(item.time_read);
    assert_within_2_seconds(item.time_updated, item.time_read);

    let res = client.readd(vec![&item]).await.unwrap();
    assert_one_modified_item(&res, &item);

    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_unread(&item);
    assert_within_5_seconds_of_now(item.time_updated);
}

#[tokio::test]
async fn favorite_and_unfavorite() {
    init();

    let client = client();

    let url = "https://en.wikipedia.org/wiki/Favorite_(disambiguation)";
    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_not_favorited(&item);

    let res = client.favorite(vec![&item]).await.unwrap();
    assert_one_not_modified_item(&res);

    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_eq!(item.favorite, FavoriteStatus::Favorited);
    assert_within_5_seconds_of_now(item.time_favorited);
    assert_within_2_seconds(item.time_updated, item.time_favorited);

    let res = client.unfavorite(vec![&item]).await.unwrap();
    assert_one_not_modified_item(&res);

    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_not_favorited(&item);
}

#[tokio::test]
async fn add_replace_and_remove_tags() {
    init();

    let client = client();

    let url = "https://medium.com/makingtuenti/we-made-the-impossible-possible-in-the-tuenti-challenge-8-edition-619df6d56381";

    // No tags at the beginning.
    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    let id = item.item_id.clone();
    assert_eq!(item.tags, None);

    // Add tags.
    let tag1 = String::from("tag1");
    let tag2 = String::from("tag2");
    let tags = vec![tag1.clone(), tag2.clone()];
    let res = client
        .modify(vec![libpocket::Action::TagsAdd {
            item_id: id.clone(),
            tags: &tags.clone(),
            time: now(),
        }])
        .await
        .unwrap();
    assert_one_not_modified_item(&res);

    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_eq!(item.tags, Some(expected_tags(tags, id.clone())));

    // Replace tags.
    let tag3 = String::from("tag3");
    let tag4 = String::from("tag4");
    let tags = vec![tag3, tag4];
    let res = client
        .modify(vec![libpocket::Action::TagsReplace {
            item_id: id.clone(),
            tags: &tags,
            time: now(),
        }])
        .await
        .unwrap();
    assert_one_not_modified_item(&res);
    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_eq!(item.tags, Some(expected_tags(tags.clone(), id.clone())));

    // Remove tags.
    let res = client
        .modify(vec![libpocket::Action::TagsRemove {
            item_id: id.clone(),
            tags: &tags,
            time: now(),
        }])
        .await
        .unwrap();
    assert_one_not_modified_item(&res);
    let item = lookup_item_from_given_url(&client, url).await.unwrap();
    assert_eq!(item.tags, None);
}

// Small helper function to get the expected `BTreeMap` of tags.
fn expected_tags(
    tags: impl IntoIterator<Item = String>,
    id: libpocket::ItemId,
) -> BTreeMap<String, Tag> {
    BTreeMap::from_iter(tags.into_iter().map(|tag| {
        (
            tag.clone(),
            Tag {
                item_id: id.clone(),
                tag: tag.clone(),
            },
        )
    }))
}

fn assert_one_modified_item(modify_response: &ModifyResponse, item: &Item) {
    assert!(modify_response.len() == 1);
    let modified_item = modify_response
        .get(0)
        .unwrap()
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap();
    assert_modified_item(&item, &modified_item);
}

// Asserts that the response contains one entry, indicating that the item was modified
// successfully, but the action results in the response did not contain a modified item.
fn assert_one_not_modified_item(modify_response: &ModifyResponse) {
    assert!(modify_response.len() == 1);
    let modified_item_opt = modify_response.get(0).unwrap().as_ref().unwrap();
    assert_eq!(modified_item_opt, &None);
}

fn assert_within_2_seconds(t1: u64, t2: u64) {
    let within_2_seconds = ((t1 as i64) - (t2 as i64)).abs() <= 3;

    assert!(
        within_2_seconds,
        "`t1`: {} is not within 2 seconds of `t2`: {}",
        t1, t2
    );
}

fn assert_within_5_seconds_of_now(past: u64) {
    let now = now();
    let duration = now - past;
    let within_5_seconds_of_now = duration <= 5;

    assert!(
        within_5_seconds_of_now,
        "`timestamp`: {} is not within 5 seconds of now: {}",
        past, now
    );
}

fn assert_unread(item: &Item) {
    assert_eq!(item.status, Status::Unread);
    assert_eq!(item.time_read, 0);
}

fn assert_not_favorited(item: &Item) {
    assert_eq!(item.favorite, FavoriteStatus::NotFavorited);
    assert_eq!(item.time_favorited, 0);
}

fn assert_contains_items<T: IntoIterator<Item = Item>>(reading_list: &ReadingList, items: T) {
    for item in items {
        assert_contains_item(reading_list, &item);
    }
}

fn assert_contains_item(reading_list: &ReadingList, item: &Item) {
    assert!(
        reading_list.contains_item(item),
        "`reading_list` does not contain item.
`reading list` = `{:#?}`
`item` = `{:#?}`",
        reading_list,
        item
    );
}

fn assert_contains_given_url_once(reading_list: &ReadingList, url: &str) {
    assert!(
        reading_list.contains_given_url_once(&url),
        "`reading_list` does not contain url.
`reading list` = `{:#?}`
`url` = `{}`",
        reading_list,
        &url
    );
}

fn assert_does_not_contain_given_url(reading_list: &ReadingList, url: &str) {
    assert!(
        reading_list.does_not_contain_given_url(&url),
        "`reading_list` contains url.
`reading list` = `{:#?}`
`url` = `{}`",
        reading_list,
        &url
    );
}

async fn lookup_item_from_given_url(client: &Client, given_url: &str) -> Option<Item> {
    let res = client
        .get(
            &GetInputBuilder::default()
                .state(Some(State::All))
                .search(Some(String::from(given_url)))
                .detail_type(Some(DetailType::Complete))
                .build()
                .unwrap(),
        )
        .await
        .unwrap();

    assert_contains_given_url_once(&res, given_url);

    res.find_given_url(given_url).cloned()
}

fn client() -> Client {
    let consumer_key = std::env::var("POCKET_CONSUMER_KEY").expect("POCKET_CONSUMER_KEY not set");
    let authorization_code =
        std::env::var("POCKET_AUTHORIZATION_CODE").expect("POCKET_AUTHORIZATION_CODE not set");

    Client::new(consumer_key, authorization_code)
}

#[derive(Debug, Error)]
enum TestHelperError {
    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),

    #[error("DeserializeError: {0}")]
    DeserializeError(#[from] serde_json::Error),
}

fn deserialize_resource<T: DeserializeOwned>(filename: &str) -> Result<T, TestHelperError> {
    let path = resource(filename)?;
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let value: T = serde_json::from_reader(reader)?;
    Ok(value)
}

fn resource(filename: &str) -> std::io::Result<PathBuf> {
    Ok(std::env::current_dir()?.join(Path::new(&format!("res/{}", filename))))
}

// TODO Maybe some of these are well worth exposing from lib.rs.
trait ReadingListExt {
    fn contains_item(&self, item: &Item) -> bool;
    fn contains_given_url_once(&self, url: &str) -> bool;
    fn does_not_contain_given_url(&self, url: &str) -> bool;
    fn given_url_count(&self, url: &str) -> usize;
    fn find_given_url(&self, url: &str) -> Option<&Item>;
}

impl ReadingListExt for ReadingList {
    fn contains_item(&self, item: &Item) -> bool {
        if let Some(ItemOrDeletedItem::Item(val)) = self.get(&item.item_id) {
            TestItem(&val) == TestItem(item)
        } else {
            false
        }
    }

    fn contains_given_url_once(&self, url: &str) -> bool {
        self.given_url_count(url) == 1
    }

    fn does_not_contain_given_url(&self, url: &str) -> bool {
        self.given_url_count(url) == 0
    }

    fn given_url_count(&self, url: &str) -> usize {
        self.values()
            .filter(|item_or_deleted_item| {
                if let ItemOrDeletedItem::Item(item) = item_or_deleted_item {
                    item.given_url == url.as_ref()
                } else {
                    false
                }
            })
            .count()
    }

    fn find_given_url(&self, url: &str) -> Option<&Item> {
        self.values().find_map(|item_or_deleted_item| {
            if let ItemOrDeletedItem::Item(item) = item_or_deleted_item {
                if item.given_url == url.as_ref() {
                    return Some(item);
                }
            }

            return None;
        })
    }
}

// There are some fields from `Item` that we generally do not want to compare in tests, like
// timestamps and account and state-specific data like `sort_id`. We use the newtype pattern to
// wrap an `Item` into a `TestItem` and reimplement useful traits.
#[derive(Debug)]
struct TestItem<'a>(&'a Item);

impl PartialEq for TestItem<'_> {
    fn eq(&self, other: &Self) -> bool {
        // TODO It's going to be challenging to maintain this list of fields up to date. Is there a
        // better way?
        self.0.given_url == other.0.given_url
            && self.0.resolved_url == other.0.resolved_url
            && self.0.given_title == other.0.given_title
            && self.0.resolved_title == other.0.resolved_title
            && self.0.favorite == other.0.favorite
            && self.0.status == other.0.status
            && self.0.excerpt == other.0.excerpt
            && self.0.is_article == other.0.is_article
            && self.0.has_image == other.0.has_image
            && self.0.has_video == other.0.has_video
            && self.0.word_count == other.0.word_count
            && self.0.is_index == other.0.is_index
            && self.0.lang == other.0.lang
            && self.0.top_image_url == other.0.top_image_url
            && self.0.domain_metadata == other.0.domain_metadata
            && self.0.listen_duration_estimate == other.0.listen_duration_estimate
            && self.0.time_to_read == other.0.time_to_read
            && self.0.amp_url == other.0.amp_url
            && self.0.images == other.0.images
            && self.0.videos == other.0.videos
            && self.0.authors == other.0.authors
            && self.0.tags == other.0.tags
            && self.0.image == other.0.image
    }
}

// Asserts that we modified an item by comparing what the /send endpoint returns us,
// `modified_item`, to the fields of the `Item` that we modified and from which it should be
// modelled after.
fn assert_modified_item(item: &Item, modified_item: &ModifiedItem) {
    // In theory this should be enough to confirm that we indeed modified the item we wanted to,
    // since `item_id` is a unique identifier.
    assert_eq!(modified_item.item_id, item.item_id);

    if modified_item.resolved_id != item.item_id {
        // Pocket API did not process the item yet.
        assert_eq!(&modified_item.resolved_id, "0");
    }
    match &modified_item.resolved_url {
        Some(url) => {
            assert_eq!(url, &item.resolved_url);
        }
        None => {
            // Pocket API did not process the item yet.
        }
    }
}

fn now() -> u64 {
    use std::time::SystemTime;

    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("negative elapsed time since the Unix epoch")
        .as_secs()
}
