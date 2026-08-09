/**
 * Branded (nominal) domain primitives — the TypeScript analogue of a Rust
 * newtype (semantic style rulebook, Confluence DESIGN 37519361: domain
 * primitives behind newtypes). `Brand<T, K>` is `T` widened with an
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
 *    checks the shape and returns `undefined` on failure, DD 39944194 D7's
 *    no-Option convention (`T | undefined` is TypeScript's Option here)
 *    ({@link handle}).
 *
 * This module must never import `effect`: it lives ABOVE and BELOW the
 * runes seam, and Effect is confined to `src/lib/server/**` (DD 39944194).
 */

import { ATPROTO_HANDLE, HANDLE_MAX_LEN, isPunycodeLabeled } from './handle-format';

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
 * atproto-shaped, punycode-rejected (DD 26050561) — sourced from
 * {@link import('./handle-format')} so the two validators cannot drift.
 * Returns `undefined` on any failure (DD 39944194 D7: no Option type,
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
