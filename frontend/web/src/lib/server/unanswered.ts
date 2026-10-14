import { error } from '@sveltejs/kit';
import { Effect } from 'effect';
import type { FetchFunction } from '$lib/api/client';
import { HttpStatus } from '$lib/api/http-status';
import { NetworkFailure } from './api/errors';
import type { ZurfurApi } from './api/zurfur-api';
import { runApi } from './runtime';

/**
 * The sign-in gate's own page for when Zurfur doesn't answer: fixed HTML with
 * no script, and nothing from the request written into it. "Try again" is
 * `href=""`, which the browser resolves to the address it is already on.
 */
export const UNANSWERED_PAGE = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="dark light">
<title>Zurfur</title>
</head>
<body>
<main>
<h1>Zurfur isn't answering right now.</h1>
<p><a href="">Try again</a></p>
</main>
</body>
</html>
`;

/** The error message a request that meets "isn't answering" carries; the error view never shows it. */
const UNANSWERED_MESSAGE = "Zurfur isn't answering right now.";

/**
 * Whether `request` is an enhanced form post: SvelteKit's client sends it with
 * `x-sveltekit-action: true` and reads the answer as an action result.
 */
function isEnhancedFormPost(request: Request): boolean {
	return request.method === 'POST' && request.headers.get('x-sveltekit-action') === 'true';
}

/**
 * The gate's answer when Zurfur doesn't answer, status 503: an enhanced form
 * post gets an error action result, which the client shows as the frame's
 * error view; anything else (a page, a `__data.json` request, a plain form
 * post) gets {@link UNANSWERED_PAGE}. It sets no cookie.
 */
export function unansweredResponse(request: Request): Response {
	const status = HttpStatus.ServiceUnavailable;
	if (isEnhancedFormPost(request)) {
		return Response.json({ type: 'error', error: { message: UNANSWERED_MESSAGE } }, { status });
	}
	const headers = { 'content-type': 'text/html; charset=utf-8' };
	return new Response(UNANSWERED_PAGE, { status, headers });
}

/**
 * {@link runApi} for a page's own reads: a backend that doesn't answer
 * (unreachable or out of time) fails the load with 503, which the frame's
 * error view words as "isn't answering", never a bare 500.
 */
export async function runRead<A, E>(
	fetch: FetchFunction,
	program: Effect.Effect<A, E | NetworkFailure, ZurfurApi>
): Promise<A> {
	const settled = await runApi(
		fetch,
		program.pipe(
			Effect.map((value) => ({ answered: true, value }) as const),
			Effect.catchIf(
				(failure): failure is NetworkFailure => failure instanceof NetworkFailure,
				() => Effect.succeed({ answered: false } as const)
			)
		)
	);
	if (!settled.answered) error(HttpStatus.ServiceUnavailable, "Zurfur isn't answering right now.");
	return settled.value;
}
