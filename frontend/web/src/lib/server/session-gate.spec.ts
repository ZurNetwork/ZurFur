import { isRedirect, type RequestEvent } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { fetchStub, problemResponse } from '$lib/testing/http';
import { isSessionRoute, sessionGate } from './session-gate';

type HandleInput = Parameters<typeof sessionGate>[0];

/** The signed-in visitor `/me` describes. */
const ALICE = { did: 'did:plc:alice', handle: 'alice.zurfur.app' };

/** Run the gate for a GET to `address` on `routeId`, with `/me` answering `me()`. */
async function gate(routeId: string | null, address: string, me: () => Response) {
	return gateWith(routeId, address, 'GET', me);
}

/** Run the gate for a `method` request to `address` on `routeId`, with `/me` answering `me()`. */
async function gateWith(
	routeId: string | null,
	address: string,
	method: string,
	me: () => Response,
	isDataRequest = false
) {
	const { fetch, calls } = fetchStub(me);
	const resolved = vi.fn(() => Promise.resolve(new Response('<p>private</p>')));
	const locals: App.Locals = {};
	const event = {
		route: { id: routeId },
		url: new URL(address, 'http://127.0.0.1:5174'),
		request: new Request(new URL(address, 'http://127.0.0.1:5174'), { method }),
		fetch,
		locals,
		isDataRequest
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

	it('lets the sign-out action through without a session, so it can always clear the cookie', async () => {
		const { outcome, resolved, calls } = await gateWith(
			'/(session)/logout',
			'/logout',
			'POST',
			() => {
				throw new TypeError('fetch failed');
			}
		);
		expect(outcome).toBeInstanceOf(Response);
		expect(resolved).toHaveBeenCalledOnce();
		expect(calls).toEqual([]);
	});

	it('still gates a data request posted to the sign-out route (only the action itself passes)', async () => {
		const { outcome, resolved } = await gateWith(
			'/(session)/logout',
			'/logout/__data.json?x-sveltekit-invalidated=001',
			'POST',
			anonymous,
			true
		);
		expect(isRedirect(outcome)).toBe(true);
		expect(resolved).not.toHaveBeenCalled();
	});

	it('still gates a named action posted to the sign-out route (only the default action passes)', async () => {
		const { outcome, resolved } = await gateWith(
			'/(session)/logout',
			'/logout?/other',
			'POST',
			anonymous
		);
		expect(isRedirect(outcome)).toBe(true);
		expect(resolved).not.toHaveBeenCalled();
	});

	it('still gates a plain GET of the sign-out route', async () => {
		const { outcome } = await gate('/(session)/logout', '/logout', anonymous);
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
