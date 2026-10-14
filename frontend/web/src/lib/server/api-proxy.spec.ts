import { describe, expect, it } from 'vitest';
import { carriesSessionCookie, extractSessionCookie, rewriteApiRequest } from './api-proxy';

// The browser-visible origin SvelteKit resolves in-app `fetch('/api/v1/...')` calls
// against (Caddy, in the real deployment). The function is origin-agnostic — it
// only compares the request origin to this one — so any consistent value works.
const eventOrigin = 'http://127.0.0.1:8080';
const apiUpstream = 'http://127.0.0.1:8081';

/** Build the same-origin outgoing request SvelteKit hands `handleFetch`. */
function sameOriginRequest(path: string, init?: RequestInit): Request {
	return new Request(`${eventOrigin}${path}`, init);
}

describe('rewriteApiRequest', () => {
	it('rewrites /api/* to the upstream with the /api/v1 prefix stripped', () => {
		const request = sameOriginRequest('/api/v1/health');

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: undefined,
			apiUpstream
		});

		expect(rewritten.url).toBe(`${apiUpstream}/health`);
	});

	it('preserves the query string when stripping the prefix', () => {
		const request = sameOriginRequest('/api/v1/accounts?page=2&sort=name');

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: undefined,
			apiUpstream
		});

		expect(rewritten.url).toBe(`${apiUpstream}/accounts?page=2&sort=name`);
	});

	it('maps a bare /api/v1 to the upstream root', () => {
		const request = sameOriginRequest('/api/v1');

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: undefined,
			apiUpstream
		});

		expect(rewritten.url).toBe(`${apiUpstream}/`);
	});

	it('forwards the incoming cookie header to the upstream', () => {
		const request = sameOriginRequest('/api/v1/me');
		const incomingCookie = 'zurfur.sid=abc123';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBe(incomingCookie);
	});

	it('forwards NO cookie header when the incoming request carries none', () => {
		// Even if the outgoing request somehow already had a cookie, only the
		// incoming session cookie may be forwarded — none in, none out.
		const request = sameOriginRequest('/api/v1/me', { headers: { cookie: 'stray=leak' } });

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: undefined,
			apiUpstream
		});

		expect(rewritten.headers.get('cookie')).toBeNull();
	});

	it('forwards ONLY the zurfur.sid pair from a multi-cookie header', () => {
		// Cookies are host-scoped by hostname, not port, so on 127.0.0.1 any other
		// local dev tool's cookies share the jar. Only the session cookie may ride
		// along to axum — everything else must be dropped.
		const request = sameOriginRequest('/api/v1/me');
		const incomingCookie = 'foo=bar; zurfur.sid=abc123; other=x%3Dy';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBe('zurfur.sid=abc123');
	});

	it('never forwards the browser-binding cookie: the callback reaches the backend without SvelteKit', () => {
		// Caddy proxies /signin-callback to axum verbatim, so the binding cookie's
		// one legitimate trip to the backend never passes through this rewrite.
		const request = sameOriginRequest('/api/v1/me');
		const incomingCookie = 'zurfur.signin=tok; __Host-zurfur.signin=tok2; zurfur.sid=abc123';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBe('zurfur.sid=abc123');
	});

	it("marks the rewritten request credentials: 'omit'", () => {
		// SvelteKit's server fetch skips re-attaching the cookie jar only for
		// `credentials: 'omit'`; the explicit zurfur.sid header still goes out.
		const request = sameOriginRequest('/api/v1/me');
		const incomingCookie = 'zurfur.sid=abc123';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.credentials).toBe('omit');
		expect(rewritten.headers.get('cookie')).toBe('zurfur.sid=abc123');
	});

	it('forwards NO cookie header when the incoming header has cookies but no zurfur.sid', () => {
		const request = sameOriginRequest('/api/v1/me');
		const incomingCookie = 'foo=bar; other=baz';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBeNull();
	});

	it('preserves a literal = inside the zurfur.sid value (split on first = only)', () => {
		// Session values can carry `=` (e.g. base64 padding). The defensive parse
		// splits each pair on its FIRST `=`, so the value survives intact.
		const request = sameOriginRequest('/api/v1/me');
		const incomingCookie = 'foo=bar; zurfur.sid=a=b==; other=baz';

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBe('zurfur.sid=a=b==');
	});

	it('preserves the method and body on a POST', async () => {
		const request = sameOriginRequest('/api/v1/accounts', {
			method: 'POST',
			body: '{"handle":"alice"}',
			headers: { 'content-type': 'application/json' }
		});

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: undefined,
			apiUpstream
		});

		expect(rewritten.method).toBe('POST');
		expect(rewritten.headers.get('content-type')).toBe('application/json');
		await expect(rewritten.text()).resolves.toBe('{"handle":"alice"}');
	});

	it('preserves other request headers through the rewrite', () => {
		const request = sameOriginRequest('/api/v1/health', {
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

	it('leaves a same-origin non-/api/v1 request untouched', () => {
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

	it('does NOT treat /api/v1foo as an /api/v1 match (prefix, not stem)', () => {
		const request = sameOriginRequest('/api/v1foo');

		const result = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: undefined,
			apiUpstream
		});

		expect(result).toBe(request);
		expect(result.url).toBe(`${eventOrigin}/api/v1foo`);
	});

	it('passes a cross-origin /api/v1 request through with NO cookie and credentials omitted', () => {
		// A fetch to some other host that happens to have an /api path must never be
		// rewritten to our upstream, and must NEVER receive our session cookie —
		// not even from SvelteKit, which attaches the visitor's jar to a host on the
		// app's hostname unless credentials are omitted.
		const request = new Request('https://cdn.example.com/api/thing', {
			headers: { 'x-custom': 'kept' }
		});

		const result = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: 'zurfur.sid=abc123',
			apiUpstream
		});

		expect(result.url).toBe('https://cdn.example.com/api/thing');
		expect(result.credentials).toBe('omit');
		expect(result.headers.get('x-custom')).toBe('kept');
		expect(result.headers.get('cookie')).toBeNull();
	});
});

describe('two zurfur.sid pairs fail closed', () => {
	// A sibling host under the app's domain can plant a second `zurfur.sid` with a
	// longer path, which the browser lists first. Which pair is the visitor's
	// can't be told, so none is forwarded and the request is signed out.
	const VALID = 'zurfur.sid=s3ss10n==';
	const JUNK = 'zurfur.sid=planted';

	it('forwards the one pair a header carries', () => {
		const forwarded = extractSessionCookie(`theme=dark; ${VALID}; other=1`);

		expect(forwarded).toBe(VALID);
	});

	it.each([
		['valid then junk', `${VALID}; ${JUNK}`],
		['junk then valid', `${JUNK}; ${VALID}`],
		['two identical pairs', `${VALID}; ${VALID}`],
		['two pairs among other cookies', `theme=dark; ${JUNK}; other=1; ${VALID}`],
		['three pairs', `${VALID}; ${JUNK}; zurfur.sid=`]
	])('forwards none for %s', (_case, incomingCookie) => {
		const forwarded = extractSessionCookie(incomingCookie);

		expect(forwarded).toBeUndefined();
	});

	it.each([
		['valid then junk', `${VALID}; ${JUNK}`],
		['junk then valid', `theme=dark; ${JUNK}; ${VALID}; zurfur.signin=tok`],
		['two identical pairs', `${VALID}; ${VALID}`]
	])('sends the upstream no cookie header at all for %s', (_case, incomingCookie) => {
		const request = sameOriginRequest('/api/v1/me', { headers: { cookie: 'stray=leak' } });

		const rewritten = rewriteApiRequest({ request, eventOrigin, incomingCookie, apiUpstream });

		expect(rewritten.headers.get('cookie')).toBeNull();
	});

	it('still counts a lookalike name as no session pair', () => {
		const forwarded = extractSessionCookie(`zurfur.sidx=1; ${VALID}; xzurfur.sid=2`);

		expect(forwarded).toBe(VALID);
	});

	it('still sees a session cookie in a header carrying two pairs', () => {
		const carried = carriesSessionCookie(`${VALID}; ${JUNK}`);

		expect(carried).toBe(true);
	});

	it('sees no session cookie in a header without one', () => {
		const carried = carriesSessionCookie('theme=dark; zurfur.sidx=1');

		expect(carried).toBe(false);
	});
});

describe('rewriteApiRequest keeps the request’s signal, so its time limits still stop it', () => {
	it.each([
		['an /api/v1 call', `${eventOrigin}/api/v1/me`],
		['a cross-origin request', 'https://cdn.example/x.json']
	])('on %s', (_, address) => {
		const limit = new AbortController();
		const request = new Request(address, { signal: limit.signal });

		const rewritten = rewriteApiRequest({
			request,
			eventOrigin,
			incomingCookie: 'zurfur.sid=abc',
			apiUpstream
		});
		limit.abort(new DOMException('limit', 'TimeoutError'));

		expect(rewritten).not.toBe(request);
		expect(rewritten.signal.aborted).toBe(true);
		expect((rewritten.signal.reason as DOMException).name).toBe('TimeoutError');
	});
});
