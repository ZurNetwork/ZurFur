import { describe, expect, it } from 'vitest';
import { errorCopy } from './error-copy';

describe('errorCopy', () => {
	it('says sign-out did not complete for any error on the sign-out route', () => {
		expect(errorCopy(502, '/(session)/logout').message).toBe(
			'Sign-out did not complete. Try again.'
		);
		expect(errorCopy(500, '/(session)/logout').title).toBe("You're still signed in");
	});

	it('gives every 404 elsewhere the one not-found', () => {
		expect(errorCopy(404, undefined).title).toBe('Nothing here');
		expect(errorCopy(404, '/(session)/accounts/[id]').title).toBe('Nothing here');
	});

	it('gives anything else the generic copy', () => {
		expect(errorCopy(500, '/(session)/accounts').title).toBe('Something went wrong');
	});
});
