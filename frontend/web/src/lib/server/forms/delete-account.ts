import { Schema } from 'effect';

export const deleteAccountForm = (handle: string) =>
	Schema.Struct({
		confirm: Schema.Trim.pipe(
			Schema.filter((typed) => typed === handle, {
				message: () => `Type ${handle} exactly to confirm`
			})
		)
	});
