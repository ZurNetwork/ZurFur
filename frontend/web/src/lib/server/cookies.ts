/**
 * Every cookie the backend sets, by name, and the one place the attributes the
 * frontend sets or clears them with are chosen. The backend's `ZurfurCookie`
 * is the twin; names and attributes here match it byte for byte.
 */

/** The session cookie: the id the backend's session layer issues. Its name never takes a prefix. */
export const SESSION_COOKIE = 'zurfur.sid';

/** The sign-in binding cookie's two names: host-locked by the `__Host-` prefix where cookies are `Secure`. */
const SIGN_IN_BINDING_COOKIES = {
	secure: '__Host-zurfur.signin',
	plain: 'zurfur.signin'
} as const;

/** The session cookie's name. */
export type SessionCookieName = typeof SESSION_COOKIE;

/** Either name the sign-in binding cookie goes by. */
export type SignInBindingCookieName =
	(typeof SIGN_IN_BINDING_COOKIES)[keyof typeof SIGN_IN_BINDING_COOKIES];

/** Every cookie name Zurfur sets: a closed set. */
export type ZurfurCookieName = SessionCookieName | SignInBindingCookieName;

/** The sign-in binding cookie's name where cookies are (or are not) `Secure`. */
export function signInBindingCookieName(secure: boolean): SignInBindingCookieName {
	return secure ? SIGN_IN_BINDING_COOKIES.secure : SIGN_IN_BINDING_COOKIES.plain;
}

/** Whether `name` is one of the sign-in binding cookie's names. */
export function isSignInBindingCookieName(name: string): name is SignInBindingCookieName {
	return name === SIGN_IN_BINDING_COOKIES.secure || name === SIGN_IN_BINDING_COOKIES.plain;
}

/** The path every Zurfur cookie lives on: the whole site. */
const COOKIE_PATH = '/';

/** The attributes a Zurfur cookie is set with, in SvelteKit's `cookies.set` shape. */
export interface ZurfurCookieOptions {
	readonly path: typeof COOKIE_PATH;
	readonly httpOnly: true;
	readonly sameSite: 'lax';
	readonly secure: boolean;
	readonly maxAge: number;
}

/** What clearing a Zurfur cookie needs: the path it was set on. */
export interface ZurfurCookieClearOptions {
	readonly path: typeof COOKIE_PATH;
}

/**
 * The attributes for setting a Zurfur cookie: `Path=/`, `HttpOnly`,
 * `SameSite=Lax`, `Secure` where cookies are secure, gone after `maxAge`
 * seconds. Called with no lifetime it gives what clearing one needs.
 */
export function cookieOptions(secure: boolean, maxAge: number): ZurfurCookieOptions;
export function cookieOptions(): ZurfurCookieClearOptions;
export function cookieOptions(
	secure?: boolean,
	maxAge?: number
): ZurfurCookieOptions | ZurfurCookieClearOptions {
	if (secure === undefined || maxAge === undefined) return { path: COOKIE_PATH };
	return { path: COOKIE_PATH, httpOnly: true, sameSite: 'lax', secure, maxAge };
}
