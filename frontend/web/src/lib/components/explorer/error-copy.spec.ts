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

	it('says Zurfur isn’t answering for a 503, whatever the route but sign-out', () => {
		expect(errorCopy(503, '/(session)/den/[...path]')).toEqual({
			title: "Zurfur isn't answering right now",
			message: 'Try again in a moment.'
		});
		expect(errorCopy(503, undefined).title).toBe("Zurfur isn't answering right now");
		expect(errorCopy(503, '/(session)/logout').title).toBe("You're still signed in");
	});

	it('gives anything else the generic copy', () => {
		expect(errorCopy(500, '/(session)/accounts').title).toBe('Something went wrong');
	});
});
