# `sentry-tauri`

A Tauri Plugin for improved Sentry support.

It's perfectly reasonable to use Sentry's Rust and browser SDKs separately in a
Tauri app. However, this plugin passes browser breadcrumbs and events through 
the Rust backend which has a number of advantages:

- Browser events are enriched with Rust, OS and device context
  - Events from both Rust and browser will have the same context for filtering
- Breadcrumbs are merged from both the Rust and browser SDKs
  - See what was happening in the Rust backend and the browser frontend at the
    time of the event

## Installation

This example also shows usage of
[`sentry_rust_minidump`](https://github.com/timfish/sentry-rust-minidump) which
allows you to capture minidumps for native crashes from a separate crash
reporting process.

Add the required dependencies in `Cargo.toml`:

```toml
[dependencies]
sentry-tauri = "0.2"
```
`sentry` and `sentry-rust-minidump` are re-exported by `sentry-tauri` so you
don't need to add them as dependencies.

```rust
fn main() {
    let client = sentry_tauri::init((
        "__YOUR_DSN__",
        sentry_tauri::ClientOptions {
          release: sentry_tauri::sentry::release_name!(),
          ..Default::default()
        },
    ));

    tauri::Builder::default()
        .plugin(sentry_tauri::plugin())
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
```

## The Plugin

- Injects and initialises `@sentry/browser` in every web-view
- Includes `beforeSend` and `beforeBreadcrumb` hooks that intercept events and breadcrumbs and passes
  them to the Rust SDK via the Tauri `invoke` API
- Tauri + `serde` + existing Sentry Rust types = Deserialisation mostly Just Works™️

## Example App

Clone this repository and install dependencies:

```shell
> yarn install
```

In `examples/basic-app/src-tauri/src/main.rs` replace the DSN with your DSN

Run in development mode:

```shell
> yarn example
```
