/**
 * The stable `?error=<code>` contract the axum `signin_callback` redirects
 * carry on failure (ZMVP-151 slice 1; codes are exact strings). Mapped to
 * human copy here, at the only screen that renders them.
 */
const CALLBACK_ERROR_MESSAGES: Record<string, string> = {
	denied: 'Sign-in was cancelled at your PDS.',
	invalid_callback: 'The sign-in response was malformed. Try again.',
	exchange_failed: 'Sign-in could not be completed with your PDS. Try again.'
};

const FALLBACK_MESSAGE = 'Sign-in failed. Try again.';

/** Human copy for a callback error code; unknown codes fall back rather than leak. */
export function callbackErrorMessage(code: string): string {
	return CALLBACK_ERROR_MESSAGES[code] ?? FALLBACK_MESSAGE;
}
