import { isRedirect, type Redirect } from '@sveltejs/kit';

/** A deferred computation handed to a helper to run and observe. */
type Thunk<T> = () => T;

/**
 * Run a thunk expected to throw a SvelteKit redirect and hand it back for
 * assertions; anything else thrown (or nothing thrown) fails the test.
 */
export async function expectRedirect(thunk: Thunk<unknown>): Promise<Redirect> {
	try {
		await thunk();
	} catch (thrown) {
		if (isRedirect(thrown)) return thrown;
		throw thrown;
	}
	throw new Error('expected a redirect to be thrown');
}
