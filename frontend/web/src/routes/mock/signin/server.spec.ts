import { afterEach, describe, expect, it, vi } from 'vitest';
import { isHttpError, isRedirect } from '@sveltejs/kit';
import { handleFromTrusted } from '$lib/types/brand';
import type { RequestHandler } from './$types';

interface EnvStub {
	dev: boolean;
	flag: string | undefined;
}

/**
 * Load the route AND `zurfur-api-mock` fresh, with `$app/environment` and
 * `$env/dynamic/private` stubbed to exactly the two facts
 * {@link import('$lib/server/api/zurfur-api-mock').mockModeEnabled} reads —
 * so this route's own containment (the 404 gate) is proven against a
 * controlled flag/dev combination, not whatever the test runner's ambient
 * process env happens to hold.
 */
async function loadRoute(stub: EnvStub) {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({
		dev: stub.dev,
		building: false,
		browser: false,
		version: 'test'
	}));
	vi.doMock('$env/dynamic/private', () => ({ env: { ZURFUR_WEB_MOCK: stub.flag } }));
	const route = await import('./+server');
	const mockApi = await import('$lib/server/api/zurfur-api-mock');
	return { GET: route.GET, mockApi };
}

afterEach(() => {
	vi.doUnmock('$app/environment');
	vi.doUnmock('$env/dynamic/private');
	vi.resetModules();
});

function eventFor(handle: string | undefined): Parameters<RequestHandler>[0] {
	const search = handle === undefined ? '' : `?handle=${encodeURIComponent(handle)}`;
	const url = new URL(`http://localhost/mock/signin${search}`);
	return { url } as unknown as Parameters<RequestHandler>[0];
}

/** Run a thunk expected to throw, and hand the thrown value back — used for both `error()` and `redirect()`, which both throw synchronously. */
function thrownBy(thunk: () => unknown): unknown {
	try {
		thunk();
	} catch (thrown) {
		return thrown;
	}
	throw new Error('expected GET to throw');
}

describe('GET /mock/signin', () => {
	it('404s when mock mode is off, and never touches the shared session', async () => {
		const { GET, mockApi } = await loadRoute({ dev: true, flag: undefined });
		const before = mockApi.mockSessionSnapshot();

		const thrown = thrownBy(() => GET(eventFor('test.zurfur.app')));

		if (!isHttpError(thrown)) throw new Error('expected an HttpError');
		expect(thrown.status).toBe(404);
		expect(mockApi.mockSessionSnapshot()).toEqual(before);
	});

	it('400s for a handle that fails claim-tier validation', async () => {
		const { GET } = await loadRoute({ dev: true, flag: '1' });

		const thrown = thrownBy(() => GET(eventFor('not a handle')));

		if (!isHttpError(thrown)) throw new Error('expected an HttpError');
		expect(thrown.status).toBe(400);
	});

	it('redirects home and boots the fixture visitor when no handle is given', async () => {
		const { GET, mockApi } = await loadRoute({ dev: true, flag: '1' });

		const thrown = thrownBy(() => GET(eventFor(undefined)));

		if (!isRedirect(thrown)) throw new Error('expected a redirect');
		expect(thrown.status).toBe(303);
		expect(thrown.location).toBe('/');
		expect(mockApi.mockSessionSnapshot()?.handle).toBe(handleFromTrusted('alice.zurfur.app'));
	});

	it('redirects home and mints the requested handle when one is given', async () => {
		const { GET, mockApi } = await loadRoute({ dev: true, flag: '1' });

		const thrown = thrownBy(() => GET(eventFor('erin.zurfur.app')));

		if (!isRedirect(thrown)) throw new Error('expected a redirect');
		expect(thrown.status).toBe(303);
		expect(mockApi.mockSessionSnapshot()?.handle).toBe(handleFromTrusted('erin.zurfur.app'));
	});
});
