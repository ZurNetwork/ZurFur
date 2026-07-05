// Schema-validity test for the Zurfur lexicons.
//
// Follows the ZMVP-38 convention that established `lexicons/`: the lexicon JSON
// files are the source of truth (JSON-only, no Rust codegen), and they are
// validated against the REAL atproto meta-schema — `@atproto/lexicon`'s
// `Lexicons` loader, which is the reference implementation of the lexicon
// meta-schema. This is a committed, hermetic version of the validation ZMVP-38
// ran during its build, extended by ZMVP-104 for the unified `feed.post`.
//
// The vendored `vendor/*.json` are canonical atproto core lexicons (NOT ours);
// they are present only so the external references on `app.zurfur.feed.post`
// resolve fully and offline:
//   - `com.atproto.label.defs#selfLabels` (the required maturity self-labels)
//   - `com.atproto.repo.strongRef`        (the post arm of the reply union)
// Run: `npm test` (from `lexicons/`).

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Lexicons, jsonToLex } = require('@atproto/lexicon');

const LEX_DIR = __dirname;

// Every Zurfur lexicon (app.zurfur.*.json) at the repo root of lexicons/.
const zurfurDocs = fs
  .readdirSync(LEX_DIR)
  .filter((f) => f.startsWith('app.zurfur.') && f.endsWith('.json'))
  .map((f) => ({ file: f, doc: JSON.parse(fs.readFileSync(path.join(LEX_DIR, f), 'utf8')) }));

// The canonical external lexicons feed.post depends on (vendored for offline ref resolution).
const vendorDocs = fs
  .readdirSync(path.join(LEX_DIR, 'vendor'))
  .filter((f) => f.endsWith('.json'))
  .map((f) => ({ file: f, doc: JSON.parse(fs.readFileSync(path.join(LEX_DIR, 'vendor', f), 'utf8')) }));

// Build one Lexicons instance from the whole graph. `.add()` validates each
// document against the lexicon meta-schema and THROWS on any invalid document,
// so a clean load is itself the "every lexicon is schema-valid" assertion.
// `.add()` normalizes refs in place, so feed it deep clones — the raw parsed
// docs stay pristine for the structural assertions below.
function buildLexicons() {
  const lex = new Lexicons();
  for (const { doc } of [...zurfurDocs, ...vendorDocs]) lex.add(structuredClone(doc));
  return lex;
}

// ---- Spec-agnostic: the whole graph is meta-schema-valid and self-consistent ----

test('every Zurfur lexicon is a valid atproto lexicon document', () => {
  // feed.post + embed.media + feed.defs + graph.collection + graph.defs.
  assert.ok(
    zurfurDocs.length >= 5,
    'expected at least feed.post + embed.media + feed.defs + graph.collection + graph.defs',
  );
  // Throws if any document violates the meta-schema.
  const lex = buildLexicons();
  // Every def in every document is addressable back out (main via the bare id,
  // named defs via id#name — this also covers defs-only files like feed.defs).
  for (const { doc } of zurfurDocs) {
    for (const defName of Object.keys(doc.defs)) {
      const uri = defName === 'main' ? `lex:${doc.id}` : `lex:${doc.id}#${defName}`;
      assert.doesNotThrow(() => lex.getDefOrThrow(uri), `def missing: ${uri}`);
    }
  }
});

// Walk every `ref` / union `refs` in our documents and assert each resolves to a
// real def in the loaded graph — internal (#replyRef, #didSubject, #aspectRatio,
// …) AND external/cross-file (app.zurfur.embed.media, app.zurfur.feed.defs#credit,
// com.atproto.label.defs#selfLabels, com.atproto.repo.strongRef).
test('every cross-reference resolves', () => {
  const lex = buildLexicons();
  const refs = [];
  const walk = (node, nsid) => {
    if (Array.isArray(node)) return node.forEach((n) => walk(n, nsid));
    if (!node || typeof node !== 'object') return;
    if (node.type === 'ref' && typeof node.ref === 'string') refs.push({ ref: node.ref, nsid });
    if (node.type === 'union' && Array.isArray(node.refs)) node.refs.forEach((r) => refs.push({ ref: r, nsid }));
    for (const v of Object.values(node)) walk(v, nsid);
  };
  for (const { doc } of zurfurDocs) walk(doc.defs, doc.id);
  assert.ok(refs.length > 0, 'expected some refs to check');
  for (const { ref, nsid } of refs) {
    const uri = ref.startsWith('#') ? `${nsid}${ref}` : ref;
    assert.doesNotThrow(() => lex.getDefOrThrow(`lex:${uri}`), `unresolved ref ${ref} in ${nsid}`);
  }
});

// ---- app.zurfur.feed.post: the unified record this epic finalizes ----

const post = zurfurDocs.find((d) => d.doc.id === 'app.zurfur.feed.post').doc;
const record = post.defs.main.record;
const embedMedia = zurfurDocs.find((d) => d.doc.id === 'app.zurfur.embed.media').doc;
const feedDefs = zurfurDocs.find((d) => d.doc.id === 'app.zurfur.feed.defs').doc;

test('the unified-post graph files all exist (embed.media, feed.defs)', () => {
  assert.ok(embedMedia, 'app.zurfur.embed.media.json must exist');
  assert.ok(feedDefs, 'app.zurfur.feed.defs.json must exist');
  // feed.comment was DELETED before publication (Replyable DD 30572573) — a
  // reply is a feed.post with `reply` set; there is no comment lexicon.
  assert.ok(
    !zurfurDocs.some((d) => d.doc.id === 'app.zurfur.feed.comment'),
    'app.zurfur.feed.comment must NOT exist (superseded by feed.post.reply)',
  );
});

test('feed.post is a record keyed by tid', () => {
  assert.equal(post.defs.main.type, 'record');
  assert.equal(post.defs.main.key, 'tid');
});

test('feed.post required set is exactly {createdAt, labels}; the rest optional', () => {
  assert.deepEqual([...record.required].sort(), ['createdAt', 'labels']);
  for (const f of ['text', 'embed', 'reply', 'credits']) {
    assert.ok(!record.required.includes(f), `${f} must be optional`);
  }
  assert.equal(record.properties.createdAt.format, 'datetime');
});

test('feed.post maturity labels are REQUIRED and use the atproto self-label union', () => {
  assert.ok(record.required.includes('labels'), 'labels is required at the schema (Safe = empty values)');
  assert.equal(record.properties.labels.type, 'union');
  assert.deepEqual(record.properties.labels.refs, ['com.atproto.label.defs#selfLabels']);
});

test('feed.post.text is optional with the (pending-ratification) 3000-grapheme cap', () => {
  assert.equal(record.properties.text.type, 'string');
  assert.equal(record.properties.text.maxGraphemes, 3000);
});

test('feed.post.embed refs the shared embed.media def with a required alt blob', () => {
  assert.equal(record.properties.embed.type, 'ref');
  assert.equal(record.properties.embed.ref, 'app.zurfur.embed.media');

  const media = embedMedia.defs.main;
  assert.equal(media.type, 'object');
  assert.deepEqual([...media.required].sort(), ['alt', 'blob']);

  const blob = media.properties.blob;
  assert.equal(blob.type, 'blob');
  assert.deepEqual(blob.accept, [
    'image/png',
    'image/jpeg',
    'image/webp',
    'image/gif',
    'video/mp4',
    'video/webm',
  ]);
  assert.equal(blob.maxSize, 100000000);
  assert.equal(media.properties.alt.type, 'string');
  assert.equal(media.properties.aspectRatio.ref, '#aspectRatio');
});

test('feed.post.reply is {root, parent}, each a union of post strongRef | did subject', () => {
  assert.equal(record.properties.reply.type, 'ref');
  assert.equal(record.properties.reply.ref, '#replyRef');

  const replyRef = post.defs.replyRef;
  assert.deepEqual([...replyRef.required].sort(), ['parent', 'root']);
  for (const arm of ['root', 'parent']) {
    assert.equal(replyRef.properties[arm].type, 'union');
    assert.deepEqual(replyRef.properties[arm].refs, ['com.atproto.repo.strongRef', '#didSubject']);
  }
  // The DID arm is modeled as its own object def so the union is ref-valid.
  assert.equal(post.defs.didSubject.properties.did.format, 'did');
});

test('feed.post credits are {role, did} DIDs with an open role vocabulary, capped in length', () => {
  assert.equal(record.properties.credits.type, 'array');
  assert.equal(record.properties.credits.maxLength, 32);
  assert.equal(record.properties.credits.items.ref, 'app.zurfur.feed.defs#credit');

  const credit = feedDefs.defs.credit;
  assert.deepEqual([...credit.required].sort(), ['did', 'role']);
  assert.equal(credit.properties.did.format, 'did');
  assert.ok(Array.isArray(credit.properties.role.knownValues), 'role is an open knownValues set');
  for (const r of ['artist', 'lines', 'colors', 'sketch', 'commissioner', 'writer']) {
    assert.ok(credit.properties.role.knownValues.includes(r), `role vocabulary missing ${r}`);
  }
});

// Boundary invariant (Boundary Contract 29622283 + Gallery Posts DD 29949954 §5/§9):
// Index-side facts must NOT appear on the public record — nor the deferred `snapshot`.
test('feed.post carries no Index-side / deferred fields (tags / commissionRef / medium / snapshot)', () => {
  for (const forbidden of ['tags', 'commissionRef', 'medium', 'snapshot']) {
    assert.ok(
      !Object.prototype.hasOwnProperty.call(record.properties, forbidden),
      `${forbidden} must NOT be on the public feed.post record`,
    );
  }
});

// Behavioral validation: well-formed record instances validate (exercising the
// full ref graph incl. the external selfLabels union, the cross-file embed.media
// and feed.defs#credit refs, and both reply arms), and the key constraints bite.
test('well-formed feed.post records validate; malformed ones are rejected', () => {
  const lex = buildLexicons();
  const goodBlob = {
    $type: 'blob',
    ref: { $link: 'bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku' },
    mimeType: 'image/png',
    size: 12345,
  };
  const safeLabels = { $type: 'com.atproto.label.defs#selfLabels', values: [] };
  const strongRef = {
    $type: 'com.atproto.repo.strongRef',
    uri: 'at://did:plc:aaaaaaaaaaaaaaaaaaaaaaaa/app.zurfur.feed.post/3juj7kd54zh2y',
    cid: 'bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku',
  };
  const didSubject = { $type: 'app.zurfur.feed.post#didSubject', did: 'did:plc:bbbbbbbbbbbbbbbbbbbbbbbb' };

  // A gallery post: embed + Safe (empty) labels + credits.
  const galleryPost = {
    $type: 'app.zurfur.feed.post',
    embed: { blob: goodBlob, alt: 'a study in ink', aspectRatio: { width: 3, height: 4 } },
    labels: { $type: 'com.atproto.label.defs#selfLabels', values: [{ val: 'nudity' }] },
    credits: [{ did: 'did:plc:aaaaaaaaaaaaaaaaaaaaaaaa', role: 'artist' }],
    createdAt: '2026-07-04T12:00:00Z',
  };
  // A comment/reply-post: text + reply to a post subject + Safe labels.
  const replyPost = {
    $type: 'app.zurfur.feed.post',
    text: 'love the linework',
    reply: { root: strongRef, parent: strongRef },
    labels: safeLabels,
    createdAt: '2026-07-04T12:00:00Z',
  };
  // A profile shout: text + reply to a DID subject (the did arm).
  const shout = {
    $type: 'app.zurfur.feed.post',
    text: 'welcome to zurfur!',
    reply: { root: didSubject, parent: didSubject },
    labels: safeLabels,
    createdAt: '2026-07-04T12:00:00Z',
  };

  // Records arrive as JSON; jsonToLex converts blob/CID JSON into the lex forms
  // (BlobRef, CID) the validator expects — the same path the write layer takes.
  const check = (rec) => lex.assertValidRecord('app.zurfur.feed.post', jsonToLex(rec));

  assert.doesNotThrow(() => check(galleryPost), 'gallery post should validate');
  assert.doesNotThrow(() => check(replyPost), 'reply-post (strongRef arm) should validate');
  assert.doesNotThrow(() => check(shout), 'profile shout (did arm) should validate');

  // Missing the required labels -> rejected (labels is schema-required).
  const noLabels = { ...galleryPost };
  delete noLabels.labels;
  assert.throws(() => check(noLabels), 'missing labels must be rejected');

  // Missing the required createdAt -> rejected.
  const noCreatedAt = { ...galleryPost };
  delete noCreatedAt.createdAt;
  assert.throws(() => check(noCreatedAt), 'missing createdAt must be rejected');

  // Embed missing the required alt -> rejected.
  const noAlt = { ...galleryPost, embed: { blob: goodBlob } };
  assert.throws(() => check(noAlt), 'embed missing alt must be rejected');

  // Credit missing its required did -> rejected.
  const badCredit = { ...galleryPost, credits: [{ role: 'artist' }] };
  assert.throws(() => check(badCredit), 'credit missing did must be rejected');
});
