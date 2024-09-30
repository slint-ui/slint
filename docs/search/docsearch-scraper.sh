#!/bin/bash -x
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

build_docs=false

while getopts a:bc:d:i:p:r:u: flag
do
    case "${flag}" in
        a) api=${OPTARG};;
        b) build_docs=true;;
        c) config=${OPTARG};;
        d) docs=${OPTARG};;
        i) index=${OPTARG};;
        p) port=${OPTARG};;
        r) remote=${OPTARG};;
        u) url=${OPTARG};;
    esac
done

# Get the directory of the current script
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Set defaults
if [ -z "$api" ]; then
  api=xyz
fi
if [ -z "$config" ]; then
  config="$script_dir/scraper-config.json"
fi
if [ -z "$docs" ]; then
  docs="$script_dir/../../target/slintdocs/html"
fi
if [ -z "$index" ]; then
  index=local
fi
if [ -z "$port" ]; then
  port=8108
fi
if [ -n "$remote" ]; then
  host=$remote
  server=$remote
  protocol=https
  port=443
  hostport=$remote
else
  host=localhost
  server=host.docker.internal
  protocol=http
  port=$port
  hostport=$host:$port
fi
if [ -z "$url" ]; then
  url="http:\/\/localhost:8000"
fi

# echo "Script Directory: $script_dir";
# echo "API Key: $api";
# echo "Config: $config";
# echo "Docs: $docs";
# echo "Host: $host";
# echo "Index: $index";
# echo "Port: $port";
# echo "Protocol: $protocol";
# echo "Server: $server";
# echo "Url: $url";
# echo "HostPort: $hostport";

if [[ $OSTYPE == 'darwin'* ]]; then
  sed=gsed
else
  sed=sed
fi

# Update server and api in searchbox.html and build slint docs
if $build_docs; then
  searchbox_html="$script_dir/../reference/_templates/searchbox.html"
  cp $searchbox_html temp_searchbox.html
  $sed -i "s/\$TYPESENSE_SEARCH_API_KEY/$api/g" $searchbox_html
  $sed -i "s/\$TYPESENSE_SERVER_PROTOCOL/$protocol/g" $searchbox_html
  $sed -i "s/\$TYPESENSE_INDEX_NAME/$index/g" $searchbox_html
  $sed -i "s/\$TYPESENSE_SERVER_PORT/$port/g" $searchbox_html
  $sed -i "s/\$TYPESENSE_SERVER_URL/$host/g" $searchbox_html
  cargo xtask slintdocs --show-warnings
fi

# Start http server
python3 -m http.server 80 -d $docs &
http_server_pid=$!

# Update index name in config file
cp $config temp_config.json
config=temp_config.json
$sed -i "s/\$TYPESENSE_INDEX_NAME/$index/g" $config


# Run docsearch-scraper
docker run -i \
  -e "TYPESENSE_API_KEY=$api" \
  -e "TYPESENSE_HOST=$server" \
  -e "TYPESENSE_PORT=$port" \
  -e "TYPESENSE_PROTOCOL=$protocol" \
  -e "CONFIG=$(cat $config | jq -r tostring)" \
  typesense/docsearch-scraper:0.10.0 2>&1 | tee temp_scraper_output.txt

# Kill http server
kill $http_server_pid

# Retrieve the collection name
collection_name=$(sed -n "s/.*POST \/collections\/\(${index}_[0-9]*\)\/.*/\1/p" temp_scraper_output.txt | sed -n '1p')
# echo "COLLECTION_NAME: $collection_name";

# Retrieve documents from typesense server
curl -H "X-TYPESENSE-API-KEY: $api" \
      "$protocol://$hostport/collections/$collection_name/documents/export" > temp_docs.jsonl

# Replace 'http://host.docker.internal' with 'http://localhost:8000' in temp_docs.jsonl
$sed -i "s/http:\/\/host.docker.internal/$url/g" temp_docs.jsonl
# $sed -i "s/http:\/\/host.docker.internal/https:\/\/releases.slint.dev\/master\/docs\/slint/g" temp_docs.jsonl

# Update typesense server
curl -H "X-TYPESENSE-API-KEY: $api" \
      -X POST \
      -T temp_docs.jsonl \
      "$protocol://$hostport/collections/$collection_name/documents/import?action=update"

curl "$protocol://$hostport/aliases/$index" -X PUT \
    -H "Content-Type: application/json" \
    -H "X-TYPESENSE-API-KEY: $api" -d '{
        "collection_name": "'"$collection_name"'"
    }'

# Remove temp files
if $build_docs; then
  cp temp_searchbox.html $searchbox_html
  rm temp_searchbox.html
fi
rm temp_docs.jsonl
rm temp_config.json
rm temp_scraper_output.txt
