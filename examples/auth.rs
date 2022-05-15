//! # Authentication
//!
//! This example program shows how to authenticate to Pocket's API starting from a platform
//! consumer key.
//! The program requests and prints an authorization code which, together with the consumer key,
//! can be used to instantiate a client to interact with Pocket's API.
//!
//! For details, see [the documentation on authentication].
//!
//! [the documentation on authentication]: https://getpocket.com/developer/docs/authentication

use libpocket::{authorization_url, get_authorization_code, get_request_token, AuthError, Client};
use std::io::Write;

fn read_line() -> String {
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Could not read line from terminal");
    input
}

fn prompt_consumer_key() -> String {
    match std::env::var("POCKET_CONSUMER_KEY") {
        Ok(val) => val,
        Err(_) => {
            print!("Please, type in your consumer key: ");
            std::io::stdout()
                .flush()
                .expect("Could not write message to terminal");

            read_line()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), AuthError> {
    env_logger::init();

    let consumer_key = prompt_consumer_key();

    let request_token = get_request_token(&consumer_key).await?;

    println!("Please visit {}", authorization_url(&request_token));
    println!("Press enter after authorizing with Pocket");
    read_line(); // Discard input.

    let authorization_code = get_authorization_code(&consumer_key, request_token).await?;

    println!(r#"export POCKET_CONSUMER_KEY="{}""#, &consumer_key);
    println!(
        r#"export POCKET_AUTHORIZATION_CODE="{}""#,
        &authorization_code
    );

    let _client = Client::new(consumer_key, authorization_code);

    Ok(())
}
