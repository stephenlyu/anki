#!/bin/bash

version=$1

script_dir=$(cd $(dirname $0) && pwd)
cd $script_dir/..

# ./ninja extract:protoc
# cargo install --path rslib/sync
cd $script_dir

mkdir -p tmp
cp ~/.cargo/bin/anki-sync-server tmp/

docker_tag=wg.ksyunkcr.com/cloudac/anki-sync-server:$version

docker build -t $docker_tag .
# docker save -o app-$version.tar $docker_tag
