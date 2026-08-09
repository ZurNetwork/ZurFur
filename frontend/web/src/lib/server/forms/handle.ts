import { Schema } from 'effect';
import { ATPROTO_HANDLE, HANDLE_MAX_LEN, isPunycodeLabeled } from '$lib/types/handle-format';

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
		Schema.filter((h) => !isPunycodeLabeled(h), {
			message: () => 'Punycode (xn--) labels are not allowed'
		})
	);
