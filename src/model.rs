use serde::de::{self, Deserialize, Deserializer, Unexpected};
use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::BTreeMap;

pub type ItemId = String;

/// A Pocket item.
/// The official API docs state that all members are optional. However, empirically it seems safe
/// to assume that the ones that are not `Option`s are always present.
#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Item {
    /// A unique identifier matching the saved item. This id must be used to perform any actions
    /// through the v3/modify endpoint.
    pub item_id: ItemId,

    /// A unique identifier similar to the item_id but is unique to the actual url of the saved
    /// item. The resolved_id identifies unique urls. For example a direct link to a New York Times
    /// article and a link that redirects (ex a shortened bit.ly url) to the same article will
    /// share the same resolved_id. If this value is 0, it means that Pocket has not processed the
    /// item. Normally this happens within seconds but is possible you may request the item before
    /// it has been resolved.
    pub resolved_id: String,

    /// The actual url that was saved with the item. This url should be used if the user wants to
    /// view the item.
    pub given_url: String,

    /// The final url of the item. For example if the item was a shortened bit.ly link, this will
    /// be the actual article the url linked to.
    pub resolved_url: String,

    /// The title that was saved along with the item.
    pub given_title: String,

    /// The title that Pocket found for the item when it was parsed.
    pub resolved_title: String,

    /// Whether the item is favorited or not.
    pub favorite: FavoriteStatus,

    /// Whether the item is unread or read (i.e. in the "Archive").
    pub status: Status,

    /// The first few lines of the item (articles only).
    pub excerpt: String,

    /// Whether the item is an article or not.
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    pub is_article: bool,

    /// Whether the item has/is an image.
    pub has_image: HasImage,

    /// Whether the item has/is a video.
    pub has_video: HasVideo,

    /// How many words are in the article.
    #[serde_as(as = "DisplayFromStr")]
    pub word_count: u64,

    // The following are not documented in the official API docs, but they are present in the
    // responses. The ones marked as Option are *sometimes* present in the responses. Use at your
    // own risk.
    /// UNIX timestamp when the item was added.
    #[serde_as(as = "DisplayFromStr")]
    pub time_added: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub time_updated: u64,

    /// UNIX timestamp when the item was read (i.e. moved to the "Archive"). Set to 0 if the item
    /// has not been read.
    #[serde_as(as = "DisplayFromStr")]
    pub time_read: u64,

    /// UNIX timestamp when the item was favorited. Set to 0 if the item has not been favorited.
    #[serde_as(as = "DisplayFromStr")]
    pub time_favorited: u64,

    /// This seems to determine the order in which items are sorted when presented to the user by
    /// client applications.
    pub sort_id: u32,

    #[serde(deserialize_with = "deserialize_string_to_bool")]
    pub is_index: bool,

    /// Language code. This is sometimes set to an empty string.
    pub lang: String,

    pub top_image_url: Option<String>,
    pub domain_metadata: Option<DomainMetadata>,
    pub listen_duration_estimate: u64,
    pub time_to_read: Option<u64>,
    pub amp_url: Option<String>,

    // The following fields are documented in the official API docs and only present when
    // detailType=complete.
    pub images: Option<BTreeMap<String, Image>>,
    pub videos: Option<BTreeMap<String, Video>>,
    pub authors: Option<BTreeMap<String, Author>>,
    pub tags: Option<BTreeMap<String, Tag>>,

    // The following are not documented in the official API docs, but they are present in responses
    // when detailType=complete.
    pub image: Option<MainImage>,
}

/// A Pocket item returned by the /v3/send endpoint, returned when *successfully* adding or
/// readding an item.
///
/// There are no official API docs stating what the endpoint returns. However, empirically it seems
/// safe to assume that the members that are not `Option`s are always present.
#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct ModifiedItem {
    /// A unique identifier matching the saved item. This id must be used to perform any actions
    /// through the v3/modify endpoint.
    pub item_id: ItemId,

    /// A unique identifier similar to the item_id but is unique to the actual url of the saved
    /// item. The resolved_id identifies unique urls. For example a direct link to a New York Times
    /// article and a link that redirects (ex a shortened bit.ly url) to the same article will
    /// share the same resolved_id. If this value is 0, it means that Pocket has not processed the
    /// item. Normally this happens within seconds but is possible you may request the item before
    /// it has been resolved.
    pub resolved_id: String,

    /// The actual url that was saved with the item. This url should be used if the user wants to
    /// view the item.
    /// Observe that it is an `Option`, unlike in `Item`.
    // Readd https://getpocket.com/developer/docs/v3/modify#action_archive returns given_url: null.
    pub given_url: Option<String>,

    /// The final url of the item. For example if the item was a shortened bit.ly link, this will
    /// be the actual article the url linked to.
    pub resolved_url: String,

    // The title that was saved along with the item.
    // TODO I guess the API would return this if we set a title when adding the item.
    // pub given_title: String,

    // The title that Pocket found for the item when it was parsed.
    // TODO I guess the API would return this if we set a title when adding the item.
    // pub resolved_title: String,
    /// The first few lines of the item (articles only).
    pub excerpt: String,

    /// Whether the item is an article or not.
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    pub is_article: bool,

    /// Whether the item has/is an image.
    pub has_image: HasImage,

    /// Whether the item has/is a video.
    pub has_video: HasVideo,

    /// How many words are in the article.
    #[serde_as(as = "DisplayFromStr")]
    pub word_count: u64,

    /// Language code. This is sometimes set to an empty string.
    /// Observe that it is an `Option`, unlike in `Item`.
    // Add httpbin.org returns lang: null.
    pub lang: Option<String>,
    pub domain_metadata: Option<DomainMetadata>,
    // TODO The API returns empty arrays so we can't parse them into these types.
    // pub images: Option<BTreeMap<String, Image>>,
    // pub videos: Option<BTreeMap<String, Video>>,
    // pub authors: Option<BTreeMap<String, Author>>,

    // TODO I guess the API would return this if we set tags when adding the item.
    // pub tags: Option<BTreeMap<String, Tag>>,

    // TODO Many more fields...
}

/// An `Item` that should be deleted.
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DeletedItem {
    pub item_id: ItemId,
    // Pocket also returns a "status" field which is set to 2, meaning "this item should be
    // deleted", as documented in the docs.
    // For some reason, Pocket's API also returns a "listen_duration_estimate" field.
    // We ignore those two fields here.
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum ItemOrDeletedItem {
    Item(Item),
    DeletedItem(DeletedItem),
}

fn deserialize_string_to_bool<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    match String::deserialize(deserializer)?.as_ref() {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(de::Error::invalid_value(Unexpected::Str(other), &"0 or 1")),
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DomainMetadata {
    pub name: Option<String>,
    pub logo: String,
    pub greyscale_logo: String,
}

/// The main image associated with an `Item`.
/// Same as an `Image`, except the `image_id`, `credit`, and `caption` fields are not present.
#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct MainImage {
    /// The `Item`'s `item_id` this image is associated with.
    pub item_id: String,

    /// A URL where the image is found.
    pub src: String,

    /// Image width.
    #[serde_as(as = "DisplayFromStr")]
    pub width: u32,

    /// Image height.
    #[serde_as(as = "DisplayFromStr")]
    pub height: u32,
}

/// An image associated with an `Item`.
#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Image {
    /// The `Item`'s `item_id` this image is associated with.
    pub item_id: String,

    /// An id for the image. An incremental integer.
    pub image_id: String,

    /// A URL where the image is found.
    pub src: String,

    /// Image width. Caution: often set to zero.
    #[serde_as(as = "DisplayFromStr")]
    pub width: u32,

    /// Image height. Caution: often set to zero.
    #[serde_as(as = "DisplayFromStr")]
    pub height: u32,

    /// Image attribution. Caution: often set to an empty string.
    // TODO This field is set to an empty string instead of removed from the response. Change the
    // model to have it be of type Option<String>.
    pub credit: String,

    /// Image caption. Caution: often set to an empty string.
    // TODO This field is set to an empty string instead of removed from the response. Change the
    // model to have it be of type Option<String>.
    pub caption: String,
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Video {
    /// The `Item`'s `item_id` this video is associated with.
    pub item_id: String,

    /// An id for the video. An incremental integer.
    pub video_id: String,

    /// A URL where the video is found.
    pub src: String,

    /// Image width. Caution: often set to zero.
    #[serde_as(as = "DisplayFromStr")]
    pub width: u32,

    /// Image height. Caution: often set to zero.
    #[serde_as(as = "DisplayFromStr")]
    pub height: u32,

    // TODO What is this? It seems to be set to 1, 2, 4, 5 or 7.
    #[serde_as(as = "DisplayFromStr")]
    #[serde(rename = "type")]
    video_type: u32,

    /// Seems to be set to YouTube/Vimeo video id. Caution: often set to an empty string.
    // TODO This field is set to an empty string instead of removed from the response. Change the
    // model to have it be of type Option<String>.
    pub vid: String,

    /// Video length in seconds. Caution: often set to Some(0).
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub length: Option<u32>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Author {
    /// The `Item`'s `item_id` this author is associated with.
    pub item_id: String,

    /// An id for the author.
    pub author_id: String,

    /// Author's name.
    pub name: String,

    /// Author's URL. This may be the author's profile page in blogging platforms like e.g. Medium
    /// or social networks like Facebook/Google+. Caution: can be an empty string.
    // TODO This field is set to an empty string instead of removed from the response. Change the
    // model to have it be of type Option<String>.
    pub url: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Tag {
    /// The `Item`'s `item_id` this tag is applied to.
    pub item_id: String,

    /// Tag name.
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FavoriteStatus {
    #[serde(rename = "0")]
    NotFavorited,
    #[serde(rename = "1")]
    Favorited,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum Status {
    #[serde(rename = "0")]
    Unread,
    #[serde(rename = "1")]
    Read,
    #[serde(rename = "2")]
    ShouldBeDeleted,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum HasImage {
    #[serde(rename = "0")]
    No,
    #[serde(rename = "1")]
    Yes,
    #[serde(rename = "2")]
    IsImage,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum HasVideo {
    #[serde(rename = "0")]
    No,
    #[serde(rename = "1")]
    Yes,
    #[serde(rename = "2")]
    IsVideo,
}

pub type ReadingList = BTreeMap<ItemId, ItemOrDeletedItem>;
