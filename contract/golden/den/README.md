# Den golden responses

Real answers of the Den routes (`den.proto`) for the web's decoder tests. They are a test
fixture, not a promise: they follow the contract and the fixture world, and change with them.

## How they are made

`backend/crates/api/tests/den_golden.rs` serves the API in process on the in-memory fixture
world (`adapter_mem::fixture`), signs in as alice and as bob (and once as no one), requests each
path, and records each answer's body. It checks that every body decodes as its generated message and re-encodes
to the same bytes, and it generates everything twice and fails if the two differ.

- **Regenerate:** `just den-golden`. It rewrites every file here and removes any file it no
  longer makes.
- **Drift check:** the same test, run by `just gate`, `just gate-light` and CI with the switch
  pinned off (only `DEN_GOLDEN=write` writes), fails
  when a committed file differs from a fresh generation, and names the recipe.

## Which files are which

- `alice/`, `bob/`, `anonymous/`: **generated.** Recorded from the served routes as that viewer
  (`anonymous/` holds the `401` answer to a request with no session). Never edit them by hand:
  change the code, the contract or the fixture world, then regenerate.
- `handwritten/`: **hand-written.** No 0.1 data produces a card (a node the viewer may know
  exists but can't open), so these are built by hand through the generated types in
  `den_golden.rs`, over the generated answers they extend. To change one, edit its builder
  there and regenerate; never edit the file.
  - `listing-with-card.json`: alice's `commissions` with a card entry, "Hidden commission".
  - `listing-with-card-mount.json`: alice's `accounts` with a card mount, "Locked Studio".
  - `card-node.json`: a card as the node of a path, with its crumbs and no content.
- `index.json`: every file above with its `source` (`generated` or `handwritten`), `viewer`,
  `method`, `path` (with its query), `status`, `contentType` and `message` (the generated
  message its body decodes as). A hand-written entry's request is the one it stands for; no
  route answers it.

## What is not here

No page with a `nextPageToken`: a page holds 50 entries, and no fixture listing reaches that.

## The format

One canonical JSON form for every file: keys sorted at every depth, pretty-printed with two
spaces, newline-terminated. A `path` is the API's own path, which the web's upstream client
requests; the contract's public spelling prefixes it with `/api/v1`.
