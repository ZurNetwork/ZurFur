import type { RequestEvent } from '@sveltejs/kit';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

/** The configured first party in these cases. */
const FIRST_PARTY = 'http://127.0.0.1:8080';

/** The signed-in visitor the backend's `/me` names when the gate asks. */
const ALICE = { did: 'did:plc:alice', handle: 'alice.zurfur.app', displayName: 'Alice' };

/**
 * Load `hooks.server` and this route fresh, in a dev build with
 * ZURFUR_PUBLIC_URL set to {@link FIRST_PARTY}: `handle` is built exactly as
 * the hook module builds it.
 */
async function freshModules() {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({
		dev: true,
		building: false,
		browser: false,
		version: 'test'
	}));
	vi.doMock('$env/dynamic/private', () => ({ env: { ZURFUR_PUBLIC_URL: FIRST_PARTY } }));
	const hooks = await import('../../../hooks.server');
	const route = await import('./+page.server');
	return { handle: hooks.handle, route };
}

type ActionEvent = Parameters<
	Awaited<ReturnType<typeof freshModules>>['route']['actions']['default']
>[0];

/** What `handle` did with a post: its response, and whether the action ran. */
interface Outcome {
	readonly response: Response;
	readonly actionRan: boolean;
}

/** Post a report to `/report-a-bug` from `origin`, signed in, through `handle`. */
async function postFrom(origin: string): Promise<Outcome> {
	const { handle, route } = await freshModules();
	const action = vi.spyOn(route.actions, 'default');
	const url = 'http://127.0.0.1:5174/report-a-bug';
	const request = new Request(url, {
		method: 'POST',
		headers: {
			'content-type': 'application/x-www-form-urlencoded',
			origin,
			cookie: 'zurfur.sid=s3ss10n'
		},
		body: 'description='
	});
	const fetch = vi.fn(() => Promise.resolve(Response.json(ALICE)));
	const event = {
		route: { id: '/(session)/report-a-bug' },
		url: new URL(url),
		request,
		fetch,
		locals: {},
		isDataRequest: false
	} as unknown as RequestEvent;
	const resolve = vi.fn(async () => {
		await route.actions.default(event as unknown as ActionEvent);
		return new Response('RESOLVED');
	});

	const response = await handle({ event, resolve });

	return { response, actionRan: action.mock.calls.length > 0 };
}

/** How long the one cold import may take on a loaded machine. */
const WARM_IMPORT_TIMEOUT_MS = 60_000;

describe('POST /report-a-bug through the root handle', () => {
	// Pay the cold transform once, so each fresh import is only an evaluation.
	beforeAll(async () => {
		await import('../../../hooks.server');
		await import('./+page.server');
	}, WARM_IMPORT_TIMEOUT_MS);

	afterEach(() => {
		vi.doUnmock('$app/environment');
		vi.doUnmock('$env/dynamic/private');
		vi.resetModules();
		vi.restoreAllMocks();
	});

	/** Silence the dev request log a dev build writes for every request. */
	function quietDevLog(): void {
		vi.spyOn(console, 'debug').mockImplementation(() => undefined);
	}

	it('refuses a cross-origin post 403, and the action never runs', async () => {
		quietDevLog();

		const outcome = await postFrom('https://evil.example');

		expect(outcome.response.status).toBe(403);
		expect(await outcome.response.text()).toBe('Cross-site POST form submissions are forbidden');
		expect(outcome.actionRan).toBe(false);
	});

	it('lets a post from the first party reach the action', async () => {
		quietDevLog();

		const outcome = await postFrom(FIRST_PARTY);

		expect(outcome.response.status).toBe(200);
		expect(outcome.actionRan).toBe(true);
	});
});
