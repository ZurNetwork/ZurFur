import type { Handle } from '@sveltejs/kit';
import { HttpStatus } from '$lib/api/http-status';
import type { FirstPartyOrigin } from '$lib/types/brand';

/** The methods that never write; every other method, in any letter case, is a write. */
const SAFE_METHODS: ReadonlySet<string> = new Set(['GET', 'HEAD', 'OPTIONS']);

/** Whether `method` is one of the safe methods, compared exactly as sent. */
function isSafe(method: string): boolean {
	return SAFE_METHODS.has(method);
}

/**
 * Whether `request` comes from `firstParty`: its `Origin` is exactly that
 * origin, or it sends no `Origin` and `Sec-Fetch-Site: same-origin`. With no
 * configured origin, nothing does.
 */
function isFirstParty(request: Request, firstParty: FirstPartyOrigin | undefined): boolean {
	if (firstParty === undefined) return false;
	const origin = request.headers.get('origin') ?? undefined;
	if (origin !== undefined) return origin === firstParty;
	return request.headers.get('sec-fetch-site') === 'same-origin';
}

/** The 403 a refused write gets, in SvelteKit's own wording, naming the method sent. */
function refusal(method: string): Response {
	return new Response(`Cross-site ${method} form submissions are forbidden`, {
		status: HttpStatus.Forbidden,
		headers: { 'content-type': 'text/plain' }
	});
}

/**
 * Refuses every write (any method but GET, HEAD and OPTIONS, any content type)
 * that doesn't come from `firstParty`, with a 403 before anything behind it
 * runs. With `firstParty` undefined, every write is refused.
 */
export function refuseCrossSiteWrites(firstParty: FirstPartyOrigin | undefined): Handle {
	return ({ event, resolve }) => {
		const { request } = event;
		if (isSafe(request.method) || isFirstParty(request, firstParty)) return resolve(event);
		return refusal(request.method);
	};
}
