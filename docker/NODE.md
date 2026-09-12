---
path: docker
charted: 2026-09-12
fs:
  - name: plc/
    role: thin Dockerfile wrapping the official @did-plc/server npm package
    node: false
---
**Is:** Dockerfiles for dev-loop-only services that don't ship an official image — currently just the local `did:plc` directory.

**Conventions:** In-memory/mock backing store, never persistent — the container is meant to be recreated (`just pds-reset` / `docker compose down -v`), not upgraded in place.

**Entry points:** `docker/plc/Dockerfile`; wired into `docker-compose.yml` at the repo root as the `plc` service.

**Refs:** none beyond the upstream did-method-plc repo cited inline in the Dockerfile.
