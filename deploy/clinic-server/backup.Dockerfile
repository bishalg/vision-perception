# syntax=docker/dockerfile:1
# owner: WP-B.18 (DPL-6..8)
#
# The `backup` service image: the backend binaries (`asd-backend migrate|admin`) plus
# pg_dump/pg_restore 16 (same major as the server), `mc`, `age`, `rclone` and the
# backup/restore scripts. Built by compose with the backend image as the extra build
# context `backend` (compose `additional_contexts: backend: docker-image://...asd-backend`),
# so the backend image must exist first:
#
#   docker compose build migrate && docker compose --profile backup build backup
#
# The build context is deploy/clinic-server; `.dockerignore` keeps `.env` out of it.

ARG MC_IMAGE=bitnamilegacy/minio-client:2024.10.8-debian-12-r1
FROM ${MC_IMAGE} AS mc

FROM postgres:16.4-bookworm
RUN apt-get update \
 && apt-get install -y --no-install-recommends age jq rclone ca-certificates coreutils \
 && rm -rf /var/lib/apt/lists/*
COPY --from=mc /opt/bitnami/minio-client/bin/mc /usr/local/bin/mc
COPY --from=backend /usr/local/bin/asd-backend /usr/local/bin/asd-backend
COPY --from=backend /usr/local/bin/asd-cli /usr/local/bin/asd-cli
COPY lib.sh backup.sh restore.sh /opt/asd/
ENV MC_CONFIG_DIR=/tmp/.mc
# Not the postgres server entrypoint: this image only runs the scripts.
ENTRYPOINT []
CMD ["/opt/asd/backup.sh", "--inside"]
