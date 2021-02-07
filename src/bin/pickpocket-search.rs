use std::env;

use pickpocket::ItemOrDeletedItem;

#[tokio::main]
async fn main() {
    let url = env::args().nth(1).expect("Expected an needle as argument");

    let client = match pickpocket::cli::client_from_env_vars() {
        Ok(client) => client,
        Err(e) => panic!("It wasn't possible to initialize a Pocket client\n{}", e),
    };

    let reading_list = client.list_all().await.unwrap();
    for (id, reading_item) in &reading_list {
        if let ItemOrDeletedItem::Item(item) = reading_item {
            if item.url().contains(&url) {
                println!(
                    "Id:\t{id}
Reading Item:\t{item:?}
Used url:\t{url}
Cleaned url:\t{clean}
",
                    id = id,
                    item = item,
                    url = item.url(),
                    clean = pickpocket::cleanup_url(item.url())
                );
            }
        }
    }
}
