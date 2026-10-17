import type { RequestEvent } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { firstPartyOrigin, type FirstPartyOrigin } from '$lib/types/brand';
import { refuseCrossSiteWrites } from './cross-site-writes';

/** The configured origin every case checks against unless it says otherwise. */
const CONFIGURED = 'http://127.0.0.1:8080';

/** Where the request arrives: deliberately not the configured origin (the vite dev port). */
const ARRIVES_AT = 'http://127.0.0.1:5174/login';

/** A form post's content type. */
const FORM_TYPE = 'application/x-www-form-urlencoded';

/** One request header, name then value. */
type HeaderPair = readonly [string, string];

/** One request to put through the hook. */
interface Visit {
	readonly url?: string;
	readonly method: string;
	readonly headers: readonly HeaderPair[];
	readonly body?: string;
}

/** What the hook did with a visit: its response, and whether anything behind it ran. */
interface Outcome {
	readonly response: Response;
	readonly resolved: boolean;
}

/** The configured origin, minted as the hook receives it. */
function configured(raw: string = CONFIGURED): FirstPartyOrigin | undefined {
	return firstPartyOrigin(raw);
}

/** Put `visit` through the hook for `firstParty`, with SvelteKit's resolve stubbed. */
async function visitWith(firstParty: FirstPartyOrigin | undefined, visit: Visit): Promise<Outcome> {
	const headers = new Headers();
	for (const [name, value] of visit.headers) headers.append(name, value);
	const url = visit.url ?? ARRIVES_AT;
	const init: RequestInit =
		visit.body === undefined
			? { method: visit.method, headers }
			: { method: visit.method, headers, body: visit.body };
	const request = new Request(url, init);
	const event = { url: new URL(url), request } as unknown as RequestEvent;
	const resolve = vi.fn(() => Promise.resolve(new Response('RESOLVED', { status: 200 })));

	const response = await refuseCrossSiteWrites(firstParty)({ event, resolve });

	return { response, resolved: resolve.mock.calls.length > 0 };
}

/** A form post to /login carrying `headers`. */
function formPost(headers: readonly HeaderPair[]): Visit {
	return {
		method: 'POST',
		headers: [['content-type', FORM_TYPE], ...headers],
		body: 'handle=alice.zurfur.app'
	};
}

/** A write with `method` from `origin`. */
function writeFrom(method: string, origin: string): Visit {
	return { method, headers: [['origin', origin]], body: 'x' };
}

/** A POST with no body, so no content type, carrying `headers` (as `sendBeacon(url)` sends). */
function bodilessPost(headers: readonly HeaderPair[]): Visit {
	return { method: 'POST', headers };
}

describe('refuseCrossSiteWrites: refused', () => {
	it.each([
		['another site', [['origin', 'https://evil.example']]],
		['a same-site neighbour on another port', [['origin', 'http://127.0.0.1:9999']]],
		['localhost on the same port', [['origin', 'http://localhost:8080']]],
		['Origin: null', [['origin', 'null']]],
		['an empty Origin', [['origin', '']]],
		[
			'two joined values',
			[
				['origin', CONFIGURED],
				['origin', 'https://evil.example']
			]
		],
		['a suffix trick', [['origin', 'http://127.0.0.1:8080.evil.example']]],
		['a longer port', [['origin', 'http://127.0.0.1:80800']]],
		['the configured origin with a trailing slash', [['origin', `${CONFIGURED}/`]]],
		[
			'an empty Origin even beside Sec-Fetch-Site: same-origin',
			[
				['origin', ''],
				['sec-fetch-site', 'same-origin']
			]
		],
		[
			'another site even beside Sec-Fetch-Site: same-origin',
			[
				['origin', 'https://evil.example'],
				['sec-fetch-site', 'same-origin']
			]
		]
	] as const)('refuses a form post from %s', async (_label, headers) => {
		const { response, resolved } = await visitWith(configured(), formPost(headers));

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});

	it('refuses a POST with no body and no content type from another site', async () => {
		const { response, resolved } = await visitWith(
			configured(),
			bodilessPost([['origin', 'https://evil.example']])
		);

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});

	it('refuses a POST with no body and no content type that sends neither header', async () => {
		const { response, resolved } = await visitWith(configured(), bodilessPost([]));

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});

	it('refuses DNS rebinding: an address and Origin that agree, but are not the configured origin', async () => {
		const rebound: Visit = {
			...formPost([['origin', 'http://evil.example:8080']]),
			url: 'http://evil.example:8080/login'
		};

		const { response, resolved } = await visitWith(configured(), rebound);

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});

	it.each(['PUT', 'PATCH', 'DELETE', 'patch', 'PROPFIND'])(
		'refuses a cross-origin %s',
		async (method) => {
			const { response, resolved } = await visitWith(
				configured(),
				writeFrom(method, 'https://evil.example')
			);

			expect(response.status).toBe(403);
			expect(resolved).toBe(false);
		}
	);

	it('refuses a cross-origin JSON post, not only form types', async () => {
		const json: Visit = {
			method: 'POST',
			headers: [
				['content-type', 'application/json'],
				['origin', 'https://evil.example']
			],
			body: '{}'
		};

		const { response, resolved } = await visitWith(configured(), json);

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});

	it('refuses every write when no origin is configured, even from the would-be origin', async () => {
		const { response, resolved } = await visitWith(undefined, formPost([['origin', CONFIGURED]]));

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});

	it('refuses every write when no origin is configured, even marked same-origin', async () => {
		const { response, resolved } = await visitWith(
			undefined,
			formPost([['sec-fetch-site', 'same-origin']])
		);

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});
});

describe('refuseCrossSiteWrites: accepted', () => {
	it('lets a post from the configured origin through, whatever address it arrived at', async () => {
		const { response, resolved } = await visitWith(
			configured(),
			formPost([['origin', CONFIGURED]])
		);

		expect(resolved).toBe(true);
		expect(response.status).toBe(200);
		expect(await response.text()).toBe('RESOLVED');
	});

	it.each(['PUT', 'PATCH', 'DELETE', 'patch', 'PROPFIND'])(
		'lets a first-party %s through',
		async (method) => {
			const { resolved } = await visitWith(configured(), writeFrom(method, CONFIGURED));

			expect(resolved).toBe(true);
		}
	);

	it('compares against the configured origin, not a default', async () => {
		const elsewhere = configured('http://127.0.0.1:9180');

		const accepted = await visitWith(elsewhere, formPost([['origin', 'http://127.0.0.1:9180']]));
		const refused = await visitWith(elsewhere, formPost([['origin', CONFIGURED]]));

		expect(accepted.resolved).toBe(true);
		expect(refused.resolved).toBe(false);
	});
});

describe('refuseCrossSiteWrites: no Origin', () => {
	it('lets a write with Sec-Fetch-Site: same-origin through', async () => {
		const { resolved } = await visitWith(
			configured(),
			formPost([['sec-fetch-site', 'same-origin']])
		);

		expect(resolved).toBe(true);
	});

	it.each(['same-site', 'cross-site', 'none'])(
		'refuses a write with Sec-Fetch-Site: %s',
		async (site) => {
			const { response, resolved } = await visitWith(
				configured(),
				formPost([['sec-fetch-site', site]])
			);

			expect(response.status).toBe(403);
			expect(resolved).toBe(false);
		}
	);

	it('refuses a write with neither header', async () => {
		const { response, resolved } = await visitWith(configured(), formPost([]));

		expect(response.status).toBe(403);
		expect(resolved).toBe(false);
	});
});

describe('refuseCrossSiteWrites: safe methods', () => {
	it.each(['GET', 'HEAD', 'OPTIONS'])('lets a %s from another site through', async (method) => {
		const read: Visit = { method, headers: [['origin', 'https://evil.example']] };

		const { response, resolved } = await visitWith(configured(), read);

		expect(resolved).toBe(true);
		expect(response.status).toBe(200);
	});
});

describe('refuseCrossSiteWrites: the refusal', () => {
	it('is a plain-text 403 naming the method sent', async () => {
		const { response } = await visitWith(configured(), writeFrom('PUT', 'https://evil.example'));

		expect(response.status).toBe(403);
		expect(response.headers.get('content-type')).toBe('text/plain');
		expect(await response.text()).toBe('Cross-site PUT form submissions are forbidden');
	});

	it('names a lowercase method as it was sent', async () => {
		const { response } = await visitWith(configured(), writeFrom('patch', 'https://evil.example'));

		expect(await response.text()).toBe('Cross-site patch form submissions are forbidden');
	});

	it('is the same for an enhanced post', async () => {
		const enhanced = formPost([
			['origin', 'https://evil.example'],
			['x-sveltekit-action', 'true'],
			['accept', 'application/json']
		]);

		const { response, resolved } = await visitWith(configured(), enhanced);

		expect(resolved).toBe(false);
		expect(response.status).toBe(403);
		expect(response.headers.get('content-type')).toBe('text/plain');
		expect(await response.text()).toBe('Cross-site POST form submissions are forbidden');
	});
});
