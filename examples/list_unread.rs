use libpocket::{Client, GetInputBuilder, ItemOrDeletedItem, State};

fn client() -> Client {
    let consumer_key = std::env::var("POCKET_CONSUMER_KEY").expect("POCKET_CONSUMER_KEY not set");
    let authorization_code =
        std::env::var("POCKET_AUTHORIZATION_CODE").expect("POCKET_AUTHORIZATION_CODE not set");

    Client::new(consumer_key, authorization_code)
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let reading_list = client()
        .get(
            &GetInputBuilder::default()
                .state(Some(State::Unread))
                .build()
                .unwrap(),
        )
        .await
        .unwrap();

    for item_or_deleted_item in reading_list.values() {
        match item_or_deleted_item {
            ItemOrDeletedItem::Item(item) => {
                println!("{} --- {}", item.resolved_title, item.resolved_url);
            }
            ItemOrDeletedItem::DeletedItem(deleted_item) => {
                println!("Item {} was deleted", deleted_item.item_id);
            }
        }
    }
}
