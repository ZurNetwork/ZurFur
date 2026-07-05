# Local `did:plc` directory server for the dev loop (ZMVP-102).
#
# The reference PDS (see docker-compose.yml `pds` service) has to submit a
# genesis operation to a live PLC directory to mint each test account's
# did:plc — there is no way to create an account without one reachable. There
# is, however, no maintained published container image for `@did-plc/server`
# (the reference did:plc implementation): the upstream repo's own Dockerfile
# is stale, targeting a monorepo layout ("packages/plc/service") the project
# no longer has since did-plc was split into its own repo
# (github.com/did-method-plc/did-method-plc). This Dockerfile is a thin
# wrapper around the OFFICIAL, published npm package instead — the same
# upstream artifact, just packaged by us since nobody else does.
#
# Runs with an IN-MEMORY (mock) database: no DATABASE_URL is set, and
# @did-plc/server falls back to `Database.mock()` when it's absent (see
# packages/server/src/bin.ts upstream). That's deliberate, not a shortcut —
# this PLC only ever needs to outlive one `docker compose up`, and wiping it
# is just recreating the container (`just pds-reset`, or `docker compose down
# -v`). Verified end-to-end against the pinned reference PDS image
# (ZURFUR_PDS_IMAGE in .env.example): boots, mints a did:plc on
# createAccount, and resolves it back.
FROM node:20-alpine

WORKDIR /app

# Pin the exact version — as of this writing also the ONLY version ever
# published to npm (dist-tag `latest` == `0.0.1`). Bump deliberately if that
# changes; verify the did:plc wire format still matches the pinned PDS build.
RUN npm install --omit=dev @did-plc/server@0.0.1

EXPOSE 2582
ENV PORT=2582

HEALTHCHECK --interval=2s --timeout=2s --retries=30 \
  CMD wget -q -O- http://localhost:2582/_health || exit 1

CMD ["node", "node_modules/@did-plc/server/dist/bin.js"]
