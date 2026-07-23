import { describe, expect, it } from 'vitest';
import { isProblem } from './problem';

describe('isProblem', () => {
	it('accepts a registry-shaped problem body', () => {
		const notAuthenticated = {
			type: 'urn:zurfur:error:not-authenticated',
			code: 'not_authenticated',
			title: 'Not authenticated',
			status: 401
		};
		expect(isProblem(notAuthenticated)).toBe(true);
	});

	it('accepts a problem carrying the optional detail member', () => {
		const withDetail = {
			type: 'urn:zurfur:error:invalid-request',
			code: 'invalid_request',
			title: 'Invalid request',
			detail: 'the handle could not be used to start sign-in',
			status: 422
		};
		expect(isProblem(withDetail)).toBe(true);
	});

	it('rejects bodies missing a required member', () => {
		const missingCode = {
			type: 'urn:zurfur:error:internal',
			title: 'Internal error',
			status: 500
		};
		expect(isProblem(missingCode)).toBe(false);
	});

	it('rejects non-object bodies', () => {
		expect(isProblem(null)).toBe(false);
		expect(isProblem('not_authenticated')).toBe(false);
		expect(isProblem(401)).toBe(false);
	});
});
