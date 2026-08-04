import { Schema } from 'effect';

/**
 * The atproto handle reference regex (https://atproto.com/specs/handle),
 * transcribed exactly — the final label may end in any alphanumeric, and the
 * pattern is case-insensitive by construction (handles are not case-sensitive;
 * the backend normalizes to lowercase, `Handle::try_new`). Punycode (`xn--`)
 * labels match this shape too — they are rejected by a separate filter in
 * {@link handleField}, not by the shape check.
 */
export const ATPROTO_HANDLE =
	/^([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/;

/**
 * The atproto spec's total-length cap, and the backend's own `HANDLE_MAX_LEN`.
 * Load-bearing beyond UX: `POST /signin` hands the handle to live resolution
 * (DNS/HTTPS), so an uncapped multi-hundred-KB "handle" is cheap-request →
 * expensive-lookup amplification. Cap it before it leaves the form.
 */
const HANDLE_MAX_LEN = 253;

/**
 * The base handle field every handle-taking form shares: trimmed, required,
 * length-capped, shape-checked against {@link ATPROTO_HANDLE}. Deliberately
 * NO punycode rule here — an `xn--` handle is a legitimate atproto identity
 * at sign-in (Engineer ruling 2026-08-05: auth-time accepts what claim-time
 * rejects). A factory because the empty-field copy differs per site.
 */
export const handleField = (emptyMessage: string) =>
	Schema.Trim.pipe(
		Schema.minLength(1, { message: () => emptyMessage }),
		Schema.maxLength(HANDLE_MAX_LEN, { message: () => 'This handle is too long' }),
		Schema.pattern(ATPROTO_HANDLE, { message: () => 'This handle is not valid' })
	);

/**
 * The claim-time handle field: {@link handleField} plus the `xn--` rejection —
 * v1 rejects punycode outright rather than allow-with-checks (DD 26050561,
 * the confusable-handles policy; its binding site is account creation, where
 * a Zurfur handle is MINTED — the backend's `Handle::try_new` is the
 * authority, this states the same rule locally).
 */
export const claimHandleField = (emptyMessage: string) =>
	handleField(emptyMessage).pipe(
		Schema.filter(
			(h) =>
				!h
					.toLowerCase()
					.split('.')
					.some((l) => l.startsWith('xn--')),
			{ message: () => 'Punycode (xn--) labels are not allowed' }
		)
	);
