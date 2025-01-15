#!/bin/sh

mkdir /data/app/log/supervisor/  -p
exec supervisord -c /etc/supervisor/supervisord.conf
