-- Zurfur's own record of a recognized visitor (see ZMVP-9, DESIGN/User). The
-- "registration that isn't": a User comes to exist on first successful sign-in,
-- keyed by the DID the visitor already owns. Identity precedes us, so we never
-- mint a DID — we recognize one.
--
-- id          UUIDv7 minted app-side (PG16 has no native uuidv7()); the
--             adapter-pg convention is an opaque internal key, supplied by the
--             application, never exposed across the public boundary.
-- did         The visitor's did:plc, sourced from Bluesky. UNIQUE enforces the
--             core rule: one DID, one User, forever. No duplicates on re-sign-in.
-- created_at  When we first recognized this DID, stored explicitly rather than
--             derived from the id — import flows can make recognition time
--             diverge from key-minting time. The application supplies it; there
--             is deliberately no DEFAULT now().
CREATE TABLE users (
    id         uuid        PRIMARY KEY,
    did        text        NOT NULL UNIQUE,
    created_at timestamptz NOT NULL
);
