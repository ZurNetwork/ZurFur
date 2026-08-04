import { Schema } from 'effect';
import { handleField } from './handle';

export { ATPROTO_HANDLE } from './handle';

/**
 * The `/login` sign-in form: one handle, validated by the shared
 * {@link handleField} (trim, length cap, atproto shape). No punycode gate at
 * sign-in — an IDN handle is an existing atproto identity authenticating,
 * not a Zurfur handle being claimed (Engineer ruling 2026-08-05; the claim
 * gate lives on `create-account.ts`). Field-level messages ride the
 * superform's `errors`; a backend `Problem` rides the same form's `message`
 * — see the action in `routes/login/+page.server.ts`.
 */
export const loginForm = Schema.Struct({
	handle: handleField('Please insert your handle')
});
