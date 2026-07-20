import { describe, expect, it } from 'vitest';
import { rewriteApiRequest } from './api-proxy';

// The browser-visible origin SvelteKit resolves in-app `fetch('/api/...')` calls
// against (Caddy, in the real deployment). The function is origin-agnostic — it
// only compares the request origin to this one — so any consistent value works.
const eventOrigin = 'http://127.0.0.1:8080';
const apiUpstream = 'http://127.0.0.1:8081';

/** Build the same-origin outgoing request SvelteKit hands `handleFetch`. */
function sameOriginRequest(path: string, init?: RequestInit): Request {
	return new Request(`${eventOrigin}${path}`, init);
}

describe('rewriteApiRequest', () => {
	it('rewrites /api/* to the upstream with the /api prefix stripped', () => {
		const request = sameOriginRequest('/api/health');

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: null,
			apiUpstream
		});

		expect(rewritten.url).toBe(`${apiUpstream}/health`);
	});

	it('preserves the query string when stripping the prefix', () => {
		const request = sameOriginRequest('/api/accounts?page=2&sort=name');

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: null,
			apiUpstream
		});

		expect(rewritten.url).toBe(`${apiUpstream}/accounts?page=2&sort=name`);
	});

	it('maps a bare /api to the upstream root', () => {
		const request = sameOriginRequest('/api');

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: null,
			apiUpstream
		});

		expect(rewritten.url).toBe(`${apiUpstream}/`);
	});

	it('forwards the incoming cookie header to the upstream', () => {
		const request = sameOriginRequest('/api/me');
		const incomingCookie = 'zurfur.sid=abc123';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBe(incomingCookie);
	});

	it('forwards NO cookie header when the incoming request carries none', () => {
		// Even if the outgoing request somehow already had a cookie, only the
		// incoming session cookie may be forwarded — none in, none out.
		const request = sameOriginRequest('/api/me', { headers: { cookie: 'stray=leak' } });

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: null,
			apiUpstream
		});

		expect(rewritten.headers.get('cookie')).toBeNull();
	});

	it('forwards ONLY the zurfur.sid pair from a multi-cookie header', () => {
		// Cookies are host-scoped by hostname, not port, so on 127.0.0.1 any other
		// local dev tool's cookies share the jar. Only the session cookie may ride
		// along to axum — everything else must be dropped.
		const request = sameOriginRequest('/api/me');
		const incomingCookie = 'foo=bar; zurfur.sid=abc123; other=x%3Dy';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBe('zurfur.sid=abc123');
	});

	it('forwards NO cookie header when the incoming header has cookies but no zurfur.sid', () => {
		const request = sameOriginRequest('/api/me');
		const incomingCookie = 'foo=bar; other=baz';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBeNull();
	});

	it('preserves a literal = inside the zurfur.sid value (split on first = only)', () => {
		// Session values can carry `=` (e.g. base64 padding). The defensive parse
		// splits each pair on its FIRST `=`, so the value survives intact.
		const request = sameOriginRequest('/api/me');
		const incomingCookie = 'foo=bar; zurfur.sid=a=b==; other=baz';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBe('zurfur.sid=a=b==');
	});

	it('preserves the method and body on a POST', async () => {
		const request = sameOriginRequest('/api/accounts', {
			method: 'POST',
			body: '{"handle":"alice"}',
			headers: { 'content-type': 'application/json' }
		});

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: null,
			apiUpstream
		});

		expect(rewritten.method).toBe('POST');
		expect(rewritten.headers.get('content-type')).toBe('application/json');
		await expect(rewritten.text()).resolves.toBe('{"handle":"alice"}');
	});

	it('preserves other request headers through the rewrite', () => {
		const request = sameOriginRequest('/api/health', {
			headers: { 'x-custom': 'kept', accept: 'application/json' }
		});

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: 'zurfur.sid=abc123',
			apiUpstream
		});

		expect(rewritten.headers.get('x-custom')).toBe('kept');
		expect(rewritten.headers.get('accept')).toBe('application/json');
	});

	it('leaves a same-origin non-/api request untouched', () => {
		const request = sameOriginRequest('/internal/data');

		const result = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: 'zurfur.sid=abc123',
			apiUpstream
		});

		// Identity: not rewritten, and (critically) no cookie forwarded off the
		// /api path.
		expect(result).toBe(request);
		expect(result.headers.get('cookie')).toBeNull();
	});

	it('does NOT treat /apifoo as an /api match (prefix, not stem)', () => {
		const request = sameOriginRequest('/apifoo');

		const result = rewriteApiRequest({ request, eventOrigin, incomingCookie: null, apiUpstream });

		expect(result).toBe(request);
		expect(result.url).toBe(`${eventOrigin}/apifoo`);
	});

	it('leaves a cross-origin /api request untouched and forwards NO cookie', () => {
		// A fetch to some other host that happens to have an /api path must never be
		// rewritten to our upstream, and must NEVER receive our session cookie.
		const request = new Request('https://cdn.example.com/api/thing');

		const result = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: 'zurfur.sid=abc123',
			apiUpstream
		});

		expect(result).toBe(request);
		expect(result.url).toBe('https://cdn.example.com/api/thing');
		expect(result.headers.get('cookie')).toBeNull();
	});
});
