/**
 * Pure, Effect-free handle-shape rules — the single source both the server
 * Effect Schema ({@link import('../server/forms/handle')}) and the frontend
 * brand validator ({@link import('./brand').handle}) build on, so the shape
 * rule cannot drift between the two call sites. No `effect` import: this
 * module lives above AND below the runes seam (DD 39944194 confines Effect
 * to `src/lib/server/**`).
 */

/**
 * The atproto handle reference regex (https://atproto.com/specs/handle),
 * transcribed exactly — the final label may end in any alphanumeric, and the
 * pattern is case-insensitive by construction (handles are not case-sensitive;
 * the backend normalizes to lowercase, `Handle::try_new`). Punycode (`xn--`)
 * labels match this shape too — they are rejected by {@link isPunycodeLabeled}
 * separately, not by the shape check.
 */
export const ATPROTO_HANDLE =
	/^([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/;

/**
 * The atproto spec's total-length cap, and the backend's own `HANDLE_MAX_LEN`.
 * Load-bearing beyond UX: `POST /signin` hands the handle to live resolution
 * (DNS/HTTPS), so an uncapped multi-hundred-KB "handle" is cheap-request →
 * expensive-lookup amplification. Cap it before it leaves the form.
 */
export const HANDLE_MAX_LEN = 253;

/**
 * True when any dot-separated label of `handle` starts with the punycode ACE
 * prefix `xn--` (case-insensitive) — DD 26050561's claim-time rejection rule,
 * factored out so the server-side claim field and the frontend brand
 * validator apply exactly the same test rather than two hand-copies that can
 * drift.
 */
export function isPunycodeLabeled(handle: string): boolean {
	return handle
		.toLowerCase()
		.split('.')
		.some((label) => label.startsWith('xn--'));
}
