import { Schema } from 'effect';

/**
 * The delete-confirmation form (WCAG 3.3.4's "Confirmed" arm): valid only
 * when the typed value, trimmed, is non-empty AND exactly the account's
 * `handle`. A factory, not a constant — the valid value is runtime data.
 * Checked server-side; the browser's `required` on the input is a courtesy,
 * never the guard. The `minLength` is load-bearing, not cosmetic: `handle` is
 * a proto3 implicit-presence string, so a wire row could carry `''` — without
 * the non-empty rule a blank confirm would then equal a blank handle and the
 * guard would degenerate to accept-anything. With it, an empty-handle row
 * simply cannot be confirm-deleted (fail closed).
 */
export const deleteAccountForm = (handle: string) =>
	Schema.Struct({
		confirm: Schema.Trim.pipe(
			Schema.minLength(1, { message: () => 'Type the handle to confirm' }),
			Schema.filter((typed) => typed === handle, {
				message: () => `Type ${handle} exactly to confirm`
			})
		)
	});
