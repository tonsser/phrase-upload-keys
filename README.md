# Phrase key uploader

A small command line app that quickly uploads multiple strings to Phrase app.

## Setup

1. Install Rust.
2. Install this with `cargo install --force --git https://github.com/tonsser/phrase-upload-keys.git`.

You also run `cargo install --force --git https://github.com/tonsser/phrase-upload-keys.git` to update to the latest version.

## Usage

Imagine you have a file called `strings.txt` that contains:

```
test.key_1
String 1

test.key_2
String 2
```

Those key/string pairs can be uploaded with

```
phrase-upload-keys strings.txt --project-name "PROJECT_NAME" --token PHRASE_ACCESS_TOKEN
```

If you don't set `--token` it will look for an environment variable called `PHRASE_ACCESS_TOKEN`.

Run `phrase-upload-keys -h` for more info.
