#!/bin/sh

cd /opt/app/ || exit

if [ -f anki-sync-server ]; then
bin1=/opt/app/anki-sync-server
exec $bin1
fi
