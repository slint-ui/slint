<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Tutorials

The source code for the Rust, C++, and Node.js versions of the Memory Game tutorial are located in
the respect rust, cpp, and node sub-directories. They are built using `mdbook`.

# Requirements

Building the tutorial requires `mdbook`, which you can install with `cargo`:

```sh
cargo install mdbook
```

# Building

To build the tutorial, enter the `rust`, `cpp`, or `node` sub-directory and run:

```sh
mdbook build
```

The output will be in the `book/html` subdirectory. To check it out, open it in your web browser.

# Code Samples

The code in the tutorial is available in separate steps in .rs, .cpp, and .js files.

The .rs files are mapped to different binaries, so you if you change into the `rust/src`
sub-directory, then `cargo run` will present you with binaries for the different steps.

The .cpp files are built using `cpp/src/CMakeLists.txt`, which is included from the top-level
`CMakeLists.txt`.

# Building search database

We use Typesense for document search.

## Infrastructure

* Typesense Server: The Typesense Server will hold the search index.
* Accessibility: The Typesense server must be accessible from the search bar in documentation site.
* Docker: Docker is needed to run the Typesense Docsearch Scraper.
* Typesense Docsearch Scraper: This tool will be used to index the documentation website.

## Pre-requisites

* Install docker (<https://docs.docker.com/engine/install/>)

* Install jq

```sh
pip3 install jq
```

## Testing Locally

* Install and start Typsense server (<https://typesense.org/docs/guide/install-typesense.html#option-2-local-machine-self-hosting>)
  * Note down the API key, the default port, and the data directory.

* Verify that the server is running
  * Replace the port below with the default port
  * It should return {"ok":true} if the server is running correctly.

```sh
curl http://localhost:8108/health
```

## Testing on Typesense Cloud

* Create an account as per instructions (<https://typesense.org/docs/guide/install-typesense.html#option-1-typesense-cloud>)
  * Note down the API key and the hostname.

## Creating search index

A helper script is located under `search` sub-folder that will (optionally) build the docs (currently only Slint docs), scrape the documents, and upload the search index to Typesense server.

The script accepts the following arguments

-a : API key to authenticate with Typesense Server (default: `xyz`)

-b : Build Slint docs (for testing locally set this flag ) (default: `false`)

-c : Location of config file (default: `docs/search/scraper-config.json`)

-d : Location of index.html of docs (default: `target/slintdocs/html`)

-i : Name of the search index (default: `local`)

-p : Port to access Typesense server (default: `8108`)

-r : Remote Server when using Typesense Cloud

-u : URL on which the docs will be served (default: `http://localhost:8000`)

Example when running locally

```sh
docs/search/docsearch-scraper.sh -b
```

Example when running on Typesense Cloud, where `$cluster_name` is the name of the cluster on Typesense Cloud

```sh
docs/search/docsearch-scraper.sh -a API_KEY -b -r TYPESENSE_CLOUD_HOST_NAME
```

## Testing search functionality

Run http server

```sh
python3 -m http.server -d target/slintdocs/html
```

Open browser (<http://localhost:8000>) and use the search bar to search for content
