import { isRedirect, type RequestEvent } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { fetchStub, problemResponse } from '$lib/testing/http';
import { isSessionRoute, sessionGate } from './session-gate';

type HandleInput = Parameters<typeof sessionGate>[0];

/** The signed-in visitor `/me` describes. */
const ALICE = { did: 'did:plc:alice', handle: 'alice.zurfur.app' };

/** Run the gate for a request to `address` on `routeId`, with `/me` answering `me()`. */
async function gate(routeId: string | null, address: string, me: () => Response) {
	const { fetch, calls } = fetchStub(me);
	const resolved = vi.fn(() => Promise.resolve(new Response('<p>private</p>')));
	const locals: App.Locals = {};
	const event = {
		route: { id: routeId },
		url: new URL(address, 'http://127.0.0.1:5174'),
		request: new Request(new URL(address, 'http://127.0.0.1:5174')),
		fetch,
		locals
	} as unknown as RequestEvent;
	const input: HandleInput = { event, resolve: resolved };
	const outcome: unknown = await Promise.resolve()
		.then(() => sessionGate(input))
		.catch((thrown: unknown) => thrown);
	return { outcome, resolved, calls, locals };
}

const anonymous = () => problemResponse(401, 'not_authenticated');
const signedIn = () => Response.json(ALICE);

describe('sessionGate', () => {
	it('refuses a crafted data request that asks to skip the layouts, before any load runs', async () => {
		const { outcome, resolved } = await gate(
			'/(session)/accounts',
			'/accounts/__data.json?x-sveltekit-invalidated=001',
			anonymous
		);
		expect(isRedirect(outcome) && outcome.location).toBe('/login');
		expect(resolved).not.toHaveBeenCalled();
	});

	it('refuses an anonymous page load and an anonymous form action alike', async () => {
		const page = await gate('/(session)/accounts', '/accounts', anonymous);
		const action = await gate('/(session)/accounts/[id]', '/accounts/a1?/delete', anonymous);
		expect([isRedirect(page.outcome), isRedirect(action.outcome)]).toEqual([true, true]);
		expect(page.resolved).not.toHaveBeenCalled();
		expect(action.resolved).not.toHaveBeenCalled();
	});

	it('refuses when the backend can’t be reached (signed-out, as the root layout reads it)', async () => {
		const { outcome } = await gate('/(session)/accounts', '/accounts', () => {
			throw new TypeError('fetch failed');
		});
		expect(isRedirect(outcome)).toBe(true);
	});

	it('lets a signed-in request through and hands the session on in locals', async () => {
		const { outcome, resolved, locals } = await gate('/(session)/accounts', '/accounts', signedIn);
		expect(outcome).toBeInstanceOf(Response);
		expect(resolved).toHaveBeenCalledOnce();
		expect(locals.session?.did).toBe('did:plc:alice');
	});

	it('leaves public routes alone, without asking the backend', async () => {
		const { resolved, calls } = await gate('/(public)/login', '/login', anonymous);
		expect(resolved).toHaveBeenCalledOnce();
		expect(calls).toEqual([]);
	});

	it('knows which routes are signed-in ones', () => {
		expect(isSessionRoute('/(session)/accounts')).toBe(true);
		expect(isSessionRoute('/(session)')).toBe(true);
		const lookalike = '/(sessionx)/a' as Parameters<typeof isSessionRoute>[0];
		expect(isSessionRoute(lookalike)).toBe(false);
		expect(isSessionRoute(null)).toBe(false);
	});
});
