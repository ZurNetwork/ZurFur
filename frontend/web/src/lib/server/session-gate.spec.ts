import { isRedirect, type RequestEvent } from '@sveltejs/kit';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FetchFunction } from '$lib/api/client';
import { fetchStub, hangingFetch, problemResponse } from '$lib/testing/http';
import { isSessionRoute, sessionGate } from './session-gate';
import { UNANSWERED_PAGE } from './unanswered';

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
	const gated = startGate({ routeId, address, method, fetch, isDataRequest });
	return { ...(await gated.outcome), calls };
}

/** One request through the gate. */
interface GateRequest {
	readonly routeId: string | null;
	readonly address: string;
	readonly method: string;
	readonly fetch: FetchFunction;
	readonly isDataRequest?: boolean;
	readonly headers?: Record<string, string>;
}

/**
 * Start the gate on `request`. Behind it sits an action that records whether
 * it ran; `outcome` settles with what the gate answered (or threw).
 */
function startGate(request: GateRequest) {
	const url = new URL(request.address, 'http://127.0.0.1:5174');
	const actionRan = { ran: false };
	const resolved = vi.fn(() => {
		actionRan.ran = true;
		return Promise.resolve(new Response('<p>private</p>'));
	});
	const locals: App.Locals = {};
	const event = {
		route: { id: request.routeId },
		url,
		request: new Request(url, { method: request.method, headers: request.headers ?? {} }),
		fetch: request.fetch,
		locals,
		isDataRequest: request.isDataRequest ?? false
	} as unknown as RequestEvent;
	const input: HandleInput = { event, resolve: resolved };
	const outcome = Promise.resolve()
		.then(() => sessionGate(input))
		.catch((thrown: unknown) => thrown)
		.then((answer: unknown) => ({ outcome: answer, resolved, locals, actionRan }));
	return { outcome };
}

/** A fetch for a backend that can't be reached at all. */
const unreachable: FetchFunction = () => Promise.reject(new TypeError('fetch failed'));

/** The response the gate answered with; fails the spec if it threw or redirected instead. */
function answered(outcome: unknown): Response {
	if (!(outcome instanceof Response))
		throw new Error(`the gate did not answer: ${String(outcome)}`);
	return outcome;
}

afterEach(() => {
	vi.useRealTimers();
});

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

	it('answers a backend it can’t reach with its own page, 503, never “signed out”', async () => {
		const gated = startGate({
			routeId: '/(session)/accounts',
			address: '/accounts',
			method: 'GET',
			fetch: unreachable
		});
		const { outcome, resolved } = await gated.outcome;

		const response = answered(outcome);
		expect(response.status).toBe(503);
		expect(response.headers.get('content-type')).toBe('text/html; charset=utf-8');
		expect(response.headers.has('set-cookie')).toBe(false);
		expect(await response.text()).toBe(UNANSWERED_PAGE);
		expect(resolved).not.toHaveBeenCalled();
	});

	it('answers a /me that never comes with its page at the session check’s 5 s', async () => {
		vi.useFakeTimers();
		let settled = false;
		const gated = startGate({
			routeId: '/(session)/den/[...path]',
			address: '/den',
			method: 'GET',
			fetch: hangingFetch().fetch
		});
		void gated.outcome.then(() => (settled = true));

		await vi.advanceTimersByTimeAsync(4_999);
		expect(settled).toBe(false);
		await vi.advanceTimersByTimeAsync(1);
		const { outcome, resolved } = await gated.outcome;

		expect(answered(outcome).status).toBe(503);
		expect(resolved).not.toHaveBeenCalled();
	});

	it('answers an enhanced form post with a 503 error result, and the action never runs', async () => {
		const gated = startGate({
			routeId: '/(session)/accounts',
			address: '/accounts',
			method: 'POST',
			fetch: unreachable,
			headers: { accept: 'application/json', 'x-sveltekit-action': 'true' }
		});
		const { outcome, actionRan } = await gated.outcome;

		const response = answered(outcome);
		expect(response.status).toBe(503);
		expect(await response.json()).toEqual({
			type: 'error',
			error: { message: "Zurfur isn't answering right now." }
		});
		expect(actionRan.ran).toBe(false);
	});

	it('answers a plain form post with its page, and the action never runs', async () => {
		const gated = startGate({
			routeId: '/(session)/accounts/[id]',
			address: '/accounts/a1?/delete',
			method: 'POST',
			fetch: unreachable
		});
		const { outcome, actionRan } = await gated.outcome;

		expect(await answered(outcome).text()).toBe(UNANSWERED_PAGE);
		expect(actionRan.ran).toBe(false);
	});

	it('answers a script navigation (__data.json) with its page, so the client loads it in full', async () => {
		const gated = startGate({
			routeId: '/(session)/den/[...path]',
			address: '/den/__data.json?x-sveltekit-invalidated=01',
			method: 'GET',
			fetch: unreachable,
			isDataRequest: true
		});
		const { outcome, resolved } = await gated.outcome;

		const response = answered(outcome);
		expect(response.status).toBe(503);
		expect(await response.text()).toBe(UNANSWERED_PAGE);
		expect(resolved).not.toHaveBeenCalled();
	});

	it('writes nothing from the address into its page', async () => {
		const gated = startGate({
			routeId: '/(session)/den/[...path]',
			address: `/den/a'b"c<script>alert(1)</script>?q='"<>`,
			method: 'GET',
			fetch: unreachable
		});
		const { outcome } = await gated.outcome;

		const body = await answered(outcome).text();
		expect(body).toBe(UNANSWERED_PAGE);
		expect(body).not.toContain('<script>');
		expect(body).not.toContain('alert');
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
