import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse } from '$lib/testing/http';
import { anonymousWhenUnreachable, getSession, startSignin } from './session';

describe('getSession', () => {
	it('returns the session for a signed-in visitor', async () => {
		const me = {
			did: 'did:plc:alice',
			handle: 'alice.zurfur.app',
			display_name: 'Alice',
			avatar_url: null
		};
		const { fetch } = fetchStub(() => Response.json(me));
		const session = await getSession(fetch);
		expect(session).toEqual(me);
	});

	it('returns null on the 401 not_authenticated problem', async () => {
		const { fetch } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const session = await getSession(fetch);
		expect(session).toBeNull();
	});

	it('throws on any other problem', async () => {
		const { fetch } = fetchStub(() => problemResponse(429, 'rate_limited'));
		await expect(getSession(fetch)).rejects.toThrow(/rate_limited/);
	});
});

describe('startSignin', () => {
	it('returns the PDS authorize location from the 303', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const { fetch } = fetchStub(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const started = await startSignin(fetch, 'alice.zurfur.app');
		expect(started).toEqual({ location: authorizeUrl });
	});

	it('returns the problem when the backend rejects the handle', async () => {
		const { fetch } = fetchStub(() => problemResponse(422, 'invalid_request'));
		const started = await startSignin(fetch, 'not a handle');
		expect('problem' in started && started.problem.code).toBe('invalid_request');
	});

	it('throws when a redirect arrives without a Location header', async () => {
		const { fetch } = fetchStub(() => new Response(null, { status: 303 }));
		await expect(startSignin(fetch, 'alice.zurfur.app')).rejects.toThrow(/no Location/);
	});

	it('rejects a problem-shaped body missing the problem content type', async () => {
		const mislabelled = new Response(
			JSON.stringify({
				type: 'urn:zurfur:error:invalid-request',
				code: 'invalid_request',
				title: 'invalid_request',
				status: 422
			}),
			{ status: 422, headers: { 'content-type': 'application/json' } }
		);
		const { fetch } = fetchStub(() => mislabelled.clone());
		await expect(startSignin(fetch, 'alice.zurfur.app')).rejects.toThrow(/contract violation/);
	});
});

describe('anonymousWhenUnreachable', () => {
	it('degrades an unreachable backend (network TypeError) to anonymous', () => {
		expect(anonymousWhenUnreachable(new TypeError('fetch failed'))).toBeNull();
	});

	it('rethrows anything else — a contract violation must not read as signed-out', () => {
		const contractViolation = new Error('API contract violation: /me responded 504');
		expect(() => anonymousWhenUnreachable(contractViolation)).toThrow(/contract violation/);
	});
});
