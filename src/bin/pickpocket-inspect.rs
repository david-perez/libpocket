use std::env;

use pickpocket::ItemOrDeletedItem;

fn main() {
    let file_name = env::args().nth(1).expect("Expected an file as argument");

    let client = match pickpocket::cli::FileClient::from_cache(&file_name) {
        Ok(client) => client,
        Err(e) => panic!("It wasn't possible to initialize a Pocket client\n{}", e),
    };

    let reading_list = client.list_all();

    for reading_item in reading_list.values() {
        if let ItemOrDeletedItem::Item(item) = reading_item {
            println!(
                "{title} | {url} | {clean} | {status}",
                url = item.url(),
                clean = pickpocket::cleanup_url(item.url()),
                title = item.title(),
                status = item.status
            );
        }
    }
}
