#!/bin/bash
# Wrapper entrypoint: copies TLS certs into place, then delegates
# to the standard postgres docker-entrypoint.
set -e

cp /certs/server.crt /var/lib/postgresql/server.crt
cp /certs/server.key /var/lib/postgresql/server.key
cp /certs/ca.crt /var/lib/postgresql/ca.crt
cp /etc/pg_hba.conf /var/lib/postgresql/pg_hba.conf

chown postgres:postgres /var/lib/postgresql/server.crt \
                        /var/lib/postgresql/server.key \
                        /var/lib/postgresql/ca.crt \
                        /var/lib/postgresql/pg_hba.conf

chmod 600 /var/lib/postgresql/server.key

exec docker-entrypoint.sh "$@"
