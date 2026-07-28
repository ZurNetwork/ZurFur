import { Schema } from 'effect';

/**
 * The delete-confirmation form (WCAG 3.3.4's "Confirmed" arm): valid only
 * when the typed value, trimmed, is exactly the account's `handle`. A factory,
 * not a constant — the valid value is runtime data. Checked server-side; the
 * browser's `required` on the input is a courtesy, never the guard.
 */
export const deleteAccountForm = (handle: string) =>
	Schema.Struct({
		confirm: Schema.Trim.pipe(
			Schema.filter((typed) => typed === handle, {
				message: () => `Type ${handle} exactly to confirm`
			})
		)
	});
