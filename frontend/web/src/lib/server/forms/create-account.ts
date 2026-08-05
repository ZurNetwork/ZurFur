import { Schema } from 'effect';
import { claimHandleField } from './handle';

/**
 * The `POST /accounts` create form: name and handle, both trimmed and
 * required. The handle takes {@link claimHandleField} — this is the CLAIM
 * site DD 26050561's punycode rejection binds to (the backend's
 * `Handle::try_new` is the authority; this is the same rule stated locally).
 * Field-level messages ride the superform's `errors`; a backend `Problem`
 * rides the same form's `message` — see the action in
 * `routes/(session)/accounts/+page.server.ts`.
 */
export const createAccountForm = Schema.Struct({
	name: Schema.Trim.pipe(Schema.minLength(1, { message: () => 'Name cannot be empty' })),
	handle: claimHandleField('Handle cannot be empty')
});
