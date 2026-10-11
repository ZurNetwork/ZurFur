/**
 * Branded (nominal) domain primitives — the TypeScript analogue of a Rust
 * newtype (domain primitives behind newtypes, never bare strings).
 * `Brand<T, K>` is `T` widened with an
 * unreachable, uniquely-keyed marker property so structurally-identical
 * strings (an `AccountId` and a `Did` are both just `string` at runtime)
 * stop being interchangeable at the type level: assigning a bare `string`
 * — or a DIFFERENT brand — where one of these is expected is a compile
 * error, even though nothing is boxed or allocated at runtime.
 *
 * Two ways a branded value comes to exist, mirroring the rulebook's own
 * constructor discipline:
 *  - a NOMINAL CAST at a TRUSTED boundary — the value already satisfies the
 *    brand's invariant (it decoded off the generated contract schema at
 *    `zurfur-api.ts`'s decode boundary), so the constructor is a plain
 *    assertion, not a check ({@link accountId}, {@link did},
 *    {@link handleFromTrusted});
 *  - a VALIDATED MINT from UNTRUSTED input — the constructor actually
 *    checks the shape and returns `undefined` on failure, this codebase's
 *    no-Option convention (`T | undefined` is TypeScript's Option here)
 *    ({@link handle}).
 *
 * This module must never import `effect`: it lives ABOVE and BELOW the
 * runes seam, and Effect is confined to `src/lib/server/**`.
 */

import { ATPROTO_HANDLE, HANDLE_MAX_LEN, isPunycodeLabeled } from './handle-format';
import { isSegment } from './segment-format';

declare const brand: unique symbol;

/** `T` branded with the nominal tag `K`, via a symbol-keyed marker property that never exists at runtime. */
export type Brand<T, K extends string> = T & { readonly [brand]: K };

/** An account's id (UUIDv7, minted by Postgres) — opaque past the decode boundary; never compared or logged as a plain string. */
export type AccountId = Brand<string, 'AccountId'>;

/** An atproto handle — shape-checked at mint ({@link handle}) or trusted at decode ({@link handleFromTrusted}). */
export type Handle = Brand<string, 'Handle'>;

/** A `did:plc` identifier — opaque past the decode boundary. */
export type Did = Brand<string, 'Did'>;

/**
 * Nominal cast for an id that already arrived validated — the trusted
 * backend decode boundary (`zurfur-api.ts`'s `decodeContract`), never
 * untrusted input. There is no runtime check to perform: the contract's
 * decoder already proved the shape (a UUIDv7 minted by Postgres), so this is
 * a plain assertion — the "nominal-cast at trusted decode" half of the rule.
 */
export function accountId(value: string): AccountId {
	return value as AccountId;
}

/**
 * Nominal cast for a DID that already arrived validated — same
 * trusted-decode rationale as {@link accountId}: the backend's `did:plc`
 * minter is the authority, not this cast.
 */
export function did(value: string): Did {
	return value as Did;
}

/**
 * Nominal cast for a handle already known valid — the trusted backend decode
 * boundary. Use {@link handle} instead for input that has NOT yet been
 * checked (a form field, a query param, …).
 */
export function handleFromTrusted(value: string): Handle {
	return value as Handle;
}

/**
 * Validated mint from UNTRUSTED input: checks the same shape rule
 * `claimHandleField` enforces server-side — trimmed, length-capped,
 * atproto-shaped, punycode (`xn--`) rejected — sourced from
 * {@link import('./handle-format')} so the two validators cannot drift.
 * Returns `undefined` on any failure (no Option type,
 * strict-null `T | undefined` is TypeScript's Option). This is a MINT, not a
 * parse — it does not report WHY input was rejected, only whether it was
 * accepted; a form wanting field-level messages uses `claimHandleField`
 * directly. The claim-time rule set is used rather than the looser sign-in
 * tier because a programmatic mint through this constructor has no
 * sign-in-vs-claim context to distinguish, and claim-time is the safer
 * default.
 */
export function handle(input: string): Handle | undefined {
	const trimmed = input.trim();
	if (trimmed.length === 0 || trimmed.length > HANDLE_MAX_LEN) return undefined;
	if (!ATPROTO_HANDLE.test(trimmed)) return undefined;
	if (isPunycodeLabeled(trimmed)) return undefined;
	return trimmed as Handle;
}

/** One piece of a path the web builds — allowlisted ASCII, never `.` or `..` ({@link pathSegment}). */
export type PathSegment = Brand<string, 'PathSegment'>;

/**
 * Validated mint from UNTRUSTED input: `piece` as a {@link PathSegment} when
 * it follows the segment rule in `segment-format.ts`, else `undefined`.
 */
export function pathSegment(piece: string): PathSegment | undefined {
	return isSegment(piece) ? (piece as PathSegment) : undefined;
}

/** A link into My Den, built by the server's Den path builder and nowhere else. */
export type DenHref = Brand<string, 'DenHref'>;

/**
 * Nominal cast for a link the Den path builder just built and checked. Only
 * that builder may call it; an eslint rule keeps every other module from
 * importing it.
 */
export function denHref(value: string): DenHref {
	return value as DenHref;
}

/** A Den node's display name: text to show, never HTML and never a key. */
export type DenName = Brand<string, 'DenName'>;

/** Nominal cast for a name off a Den read (trusted decode boundary). */
export function denName(value: string): DenName {
	return value as DenName;
}

/** A Den node's provisional type name, such as `commission`: opaque, it only picks a marker. */
export type DenType = Brand<string, 'DenType'>;

/** Nominal cast for a type name off a Den read (trusted decode boundary). */
export function denType(value: string): DenType {
	return value as DenType;
}

/** An opaque position in a Den listing, sent back to fetch the next page. */
export type PageToken = Brand<string, 'PageToken'>;

/** The longest page token the web passes on; a longer one is dropped. */
const PAGE_TOKEN_MAX_LEN = 512;

/** Visible ASCII only: a page token is opaque, but never holds spaces or control characters. */
const PAGE_TOKEN_PATTERN = /^[\x21-\x7e]+$/;

/**
 * Validated mint from UNTRUSTED input (an address's query, or a listing's
 * next-page field): 1 to 512 visible ASCII characters, else `undefined`.
 * The token stays opaque: nothing here reads what it means.
 */
export function pageToken(raw: string): PageToken | undefined {
	if (raw.length === 0 || raw.length > PAGE_TOKEN_MAX_LEN) return undefined;
	return PAGE_TOKEN_PATTERN.test(raw) ? (raw as PageToken) : undefined;
}
