import { describe, expect, it } from 'vitest';
import {
	cookieOptions,
	isSignInBindingCookieName,
	SESSION_COOKIE,
	signInBindingCookieName
} from './cookies';

describe('cookie names', () => {
	it('keeps the session cookie name the backend issues', () => {
		expect(SESSION_COOKIE).toBe('zurfur.sid');
	});

	it('host-locks the sign-in binding cookie where cookies are Secure', () => {
		expect(signInBindingCookieName(true)).toBe('__Host-zurfur.signin');
		expect(signInBindingCookieName(false)).toBe('zurfur.signin');
	});

	it('recognizes only the sign-in binding cookie names', () => {
		expect(isSignInBindingCookieName('__Host-zurfur.signin')).toBe(true);
		expect(isSignInBindingCookieName('zurfur.signin')).toBe(true);
		expect(isSignInBindingCookieName('zurfur.sid')).toBe(false);
		expect(isSignInBindingCookieName('zurfur.signinx')).toBe(false);
		expect(isSignInBindingCookieName('__Secure-zurfur.signin')).toBe(false);
	});
});

describe('cookieOptions', () => {
	it('sets a Secure cookie with every attribute the backend sets', () => {
		const expected = { path: '/', httpOnly: true, sameSite: 'lax', secure: true, maxAge: 600 };
		expect(cookieOptions(true, 600)).toStrictEqual(expected);
	});

	it('sets a plain cookie with the same attributes, Secure off', () => {
		const expected = { path: '/', httpOnly: true, sameSite: 'lax', secure: false, maxAge: 600 };
		expect(cookieOptions(false, 600)).toStrictEqual(expected);
	});

	it('clears a cookie on the path it was set on', () => {
		expect(cookieOptions()).toStrictEqual({ path: '/' });
	});
});
