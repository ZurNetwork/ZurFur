import { HttpStatus } from '$lib/api/http-status';

/** What an error page says: its title and its message, from a fixed list. */
export interface ErrorCopy {
	readonly title: string;
	readonly message: string;
}

/** The sign-out action's route: an error there means the session may still be live. */
const SIGN_OUT_ROUTE = '/(session)/logout';

const SIGN_OUT_INCOMPLETE: ErrorCopy = {
	title: "You're still signed in",
	message: 'Sign-out did not complete. Try again.'
};

const NOT_FOUND: ErrorCopy = {
	title: 'Nothing here',
	message: "This doesn't exist, or you can't open it."
};

const NOT_ANSWERING: ErrorCopy = {
	title: "Zurfur isn't answering right now",
	message: 'Try again in a moment.'
};

const SOMETHING_WENT_WRONG: ErrorCopy = {
	title: 'Something went wrong',
	message: 'Something went wrong reading this. Try again in a moment.'
};

/**
 * The copy for an error, chosen by its status and the route it happened on,
 * never by the error's own message: a failed sign-out says so, a 404 is the
 * one not-found, a 503 says Zurfur isn't answering, anything else is a calm
 * generic line.
 */
export function errorCopy(status: number, routeId: string | undefined): ErrorCopy {
	if (routeId === SIGN_OUT_ROUTE) return SIGN_OUT_INCOMPLETE;
	if (status === HttpStatus.NotFound) return NOT_FOUND;
	if (status === HttpStatus.ServiceUnavailable) return NOT_ANSWERING;
	return SOMETHING_WENT_WRONG;
}
