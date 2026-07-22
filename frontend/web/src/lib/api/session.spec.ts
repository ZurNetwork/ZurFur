import { describe, expect, it } from 'vitest';
import { getSession, startSignin } from './session';

function fetchReturning(response: Response): typeof globalThis.fetch {
	return (async () => response) as typeof globalThis.fetch;
}

function problemResponse(status: number, code: string): Response {
	const body = {
		type: `urn:zurfur:error:${code.replaceAll('_', '-')}`,
		code,
		title: code,
		status
	};
	const headers = { 'content-type': 'application/problem+json' };
	return new Response(JSON.stringify(body), { status, headers });
}

describe('getSession', () => {
	it('returns the session for a signed-in visitor', async () => {
		const me = {
			did: 'did:plc:alice',
			handle: 'alice.zurfur.app',
			display_name: 'Alice',
			avatar_url: null
		};
		const session = await getSession(fetchReturning(Response.json(me)));
		expect(session).toEqual(me);
	});

	it('returns null on the 401 not_authenticated problem', async () => {
		const session = await getSession(fetchReturning(problemResponse(401, 'not_authenticated')));
		expect(session).toBeNull();
	});

	it('throws on any other problem', async () => {
		await expect(getSession(fetchReturning(problemResponse(429, 'rate_limited')))).rejects.toThrow(
			/rate_limited/
		);
	});
});

describe('startSignin', () => {
	it('returns the PDS authorize location from the 303', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const redirect = new Response(null, { status: 303, headers: { location: authorizeUrl } });
		const started = await startSignin(fetchReturning(redirect), 'alice.zurfur.app');
		expect(started).toEqual({ location: authorizeUrl });
	});

	it('returns the problem when the backend rejects the handle', async () => {
		const started = await startSignin(
			fetchReturning(problemResponse(422, 'invalid_request')),
			'not a handle'
		);
		expect('problem' in started && started.problem.code).toBe('invalid_request');
	});

	it('throws when a redirect arrives without a Location header', async () => {
		const bareRedirect = new Response(null, { status: 303 });
		await expect(startSignin(fetchReturning(bareRedirect), 'alice.zurfur.app')).rejects.toThrow(
			/no Location/
		);
	});
});
