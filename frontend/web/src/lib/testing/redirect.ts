import { isRedirect, type Redirect } from '@sveltejs/kit';

/**
 * Run a thunk expected to throw a SvelteKit redirect and hand it back for
 * assertions; anything else thrown (or nothing thrown) fails the test.
 */
export async function expectRedirect(thunk: () => unknown): Promise<Redirect> {
	try {
		await thunk();
	} catch (thrown) {
		if (isRedirect(thrown)) return thrown;
		throw thrown;
	}
	throw new Error('expected a redirect to be thrown');
}
