/** The query parameter the sign-in page reads to say a sign-out wasn't confirmed. */
export const SIGNOUT_PARAM = 'signout';

/** Its one value: the backend refused or couldn't be reached, but this device is signed out. */
export const SIGNOUT_UNCONFIRMED = 'unconfirmed';

/** Where an unconfirmed sign-out lands: the sign-in page, saying what happened. */
export const SIGNOUT_UNCONFIRMED_LOCATION = `/login?${SIGNOUT_PARAM}=${SIGNOUT_UNCONFIRMED}`;

/** What the sign-in page says after an unconfirmed sign-out. */
export const SIGNOUT_UNCONFIRMED_NOTICE =
	"You're signed out on this device. Zurfur couldn't confirm it, so your session on the server ends when it expires.";
