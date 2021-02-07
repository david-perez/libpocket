use std::collections::BTreeSet;

use pickpocket::Status;
use pickpocket::{batch::BatchApp, ItemOrDeletedItem};

#[tokio::main]
async fn main() {
    let app = BatchApp::default();

    let mut ids: BTreeSet<&str> = BTreeSet::new();

    let cache_reading_list = app.cache_client.list_all();

    for line in app.file_lines() {
        let url = line.expect("Could not read line");
        match app.get(&url as &str) {
            Some(id) => {
                let reading_item = cache_reading_list.get(id).expect("cant locate id");
                if let ItemOrDeletedItem::Item(item) = reading_item {
                    if item.status == Status::Unread {
                        ids.insert(id);
                    } else {
                        println!("Url {} already marked as read", url);
                    }
                }
            }
            None => println!("Url {} did not match", &url),
        }
    }

    app.client.mark_as_read(ids).await;
}
