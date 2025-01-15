#!/usr/bin/env bash

version=$1

if [ -z $version ]; then
    echo version required
    exit 1
fi

dir=`pwd`

mkdir -p data/app/config
mkdir -p data/app/logs

docker service rm anki-sync-server > /dev/null 2>&1

# docker pull wg.ksyunkcr.com/cloudac/anki-sync-server:$version

docker service create --name anki-sync-server --publish published=8880,target=8080,mode=host,protocol=tcp --mount type=bind,source=$dir/data,destination=/opt/app/data --mount type=bind,source=$dir/logs,destination=/opt/app/log wg.ksyunkcr.com/cloudac/anki-sync-server:$version
