# libpocket

Async bindings to Pocket API (https://getpocket.com).

## Authentication

Note down the `POCKET_CONSUMER_KEY` you can find in Pocket's ["My
Applications"] dashboard.  Retrieve a `POCKET_AUTHORIZATION_CODE` for your
`POCKET_CONSUMER_KEY` by running the [`examples/auth.rs`](examples/auth.rs)
example program and following the instructions.

```sh
POCKET_CONSUMER_KEY="<your-pocket-consumer-key>" cargo run --example auth
```

Set and export the variables in your shell for convenience:

```sh
export POCKET_CONSUMER_KEY="<your-pocket-consumer-key>"
export POCKET_AUTHORIZATION_CODE="<your-pocket-authorization-code>"
```

["My Applications"]: https://getpocket.com/developer/apps/

## Debugging

This library integrates with the [`log`] logging façade crate. You can get
debug information by prepending `RUST_LOG=DEBUG` to executable invocations that
use this library, like the [example programs] or the tests. For example, to run
all tests with debug information and backtraces:

```sh
RUST_LOG=DEBUG RUST_BACKTRACE=1 cargo test
```

[`log`]: https://docs.rs/log
[example programs]: [examples/]

### Sample commands to hit Pocket's API directly

These might prove useful to debug HTTP responses, or to interact with Pocket's
API directly in shell script without having to use this library (they are using
[HTTPie](https://httpie.io/)):

```sh
http https://getpocket.com/v3/get \
  consumer_key="$POCKET_CONSUMER_KEY" \
  access_token="$POCKET_AUTHORIZATION_CODE" \
  state=all \
  detailType=complete \
  search="Ana Vidovic"
http https://getpocket.com/v3/send \
  consumer_key="$POCKET_CONSUMER_KEY" \
  access_token="$POCKET_AUTHORIZATION_CODE" \
  actions=[{action="add", time=0, url="https://www.rust-lang.org/"}]
http https://getpocket.com/v3/send < json_document.json
```

See Pocket's [API documentation] for more endpoints and parameters.

[API documentation]: https://getpocket.com/developer/docs

## Testing

To run the unit tests, you need to have `POCKET_CONSUMER_KEY` set.

```sh
cargo test --lib
```

To run the integration tests, you need to have both `POCKET_CONSUMER_KEY` and
`POCKET_AUTHORIZATION_CODE` set. Additionally, your Pocket account needs to:

1. have the URL https://getpocket.com/developer/docs/v3/modify#action_archive
   in its reading list; and
1. have the items [`pdf.json`](res/pdf.json), [`blog.json`](res/blog.json),
   [`video.json`](res/video.json), and
   `https://getpocket.com/developer/docs/v3/modify#action_archive` in its
   reading list (including their tags). Note that the timestamps and
   identifiers are from my Pocket test account, so you will have to change
   those after recreating the items in your account.

```sh
cargo test --test integration_tests
```

## Credits

* [Bruno Tavares] for [pickpocket].

[Bruno Tavares]: https://github.com/bltavares
[pickpocket]: https://github.com/bltavares/pickpocket
