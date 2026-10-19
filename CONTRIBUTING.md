# Contributing to Feishin

Thanks for your interest in contributing!

Check the existing [issues](https://github.com/jeffvli/feishin/issues).

## IMPORTANT

For feature contributions that do not have an existing feature request, please open an issue, discussion, or ask [Feishin discord server](https://discord.gg/FVKpcMDy5f) **BEFORE** beginning to implement it to determine whether or not the feature would be acceptable for the app. Simply submitting a large feature with thousands of new or changed lines of code without asking if the feature is desired for the application will run the risk of instant closure.

Bug fixes are likely to be accepted without discussion.

## Development environment

Feishin uses Rust 1.91 or newer and GPUI. Follow the [native development guide](docs/native/README.md) for platform dependencies, isolated profiles, packaging and tests.

```sh
cargo run --locked -p feishin-gpui
cargo fmt --all -- --check
cargo test --locked --workspace -- --test-threads=4
cargo clippy --locked --workspace --all-targets -- -D warnings
node --test scripts/native-tests/remote.cjs
```

Node 22 or newer is needed for the remote-client tests. The desktop build uses Cargo. Translation catalogs live in `crates/feishin-core/assets/locales`.

## AI Disclosure

AI-generated code **is accepted** under the following conditions:

1. You are a professional or hobbyist developer using agentic coding as a tool rather than a crutch
2. You are manually reviewing the output code before submitting the pull-request
3. You are manually testing the functionality of your code before submitting the pull-request

Pull requests are **manually reviewed** by maintainers which means that submitting any form of slop wastes our precious time that could otherwise go towards fixing bugs or implementing new features.

When communicating with maintainers or other users in issues, discussions, or pull requests, DO NOT respond to others with solely AI generated text. I and other maintainers spend REAL and VALUABLE time maintaining this project, so communicating in your own words is the bare minimum that I ask of you.

## Pull requests

Please include:

- A clear description of the change in your own words
- Screenshots or recordings for UI changes if applicable

## Translations

Translations are managed through [Weblate](https://hosted.weblate.org/projects/feishin/). You only need to include translation strings for the `en.json` file. Do not manually update the other language files as these are handled directly by volunteer translators.
