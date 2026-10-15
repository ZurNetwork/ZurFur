/**
 * SvelteKit's real server-side `event.fetch`, reached through its internals on
 * purpose: which cookies it re-attaches, and what it hands `handleFetch`, is
 * source-only behaviour, so a kit bump that changes `create_fetch` must break
 * the specs that use this loudly.
 */

import type { HandleFetch, RequestEvent } from '@sveltejs/kit';
// @ts-expect-error -- kit internal, untyped
import { create_fetch } from '../../../node_modules/@sveltejs/kit/src/runtime/server/fetch.js';
// @ts-expect-error -- kit internal, untyped
import { get_cookies } from '../../../node_modules/@sveltejs/kit/src/runtime/server/cookie.js';

/** What `event.fetch` and the app's `handleFetch` need of the request event: its address, its request, its locals and its route. */
export type FetchingEvent = Pick<RequestEvent, 'url' | 'request' | 'locals' | 'route'>;

/** `event.fetch` as SvelteKit builds it for `event`, with `hook` as the app's `handleFetch`. */
export function kitEventFetch(event: FetchingEvent, hook: HandleFetch): typeof fetch {
	/* eslint-disable @typescript-eslint/no-unsafe-assignment, @typescript-eslint/no-unsafe-call, @typescript-eslint/no-unsafe-return -- kit internals are untyped */
	const { get_cookie_header, set_internal, set_trailing_slash } = get_cookies(
		event.request,
		event.url
	);
	set_trailing_slash('never');
	return create_fetch({
		event,
		options: { hooks: { handleFetch: hook } },
		manifest: { assets: new Set(), _: { server_assets: {} }, mimeTypes: {} },
		state: { depth: 0 },
		get_cookie_header,
		set_internal
	});
	/* eslint-enable @typescript-eslint/no-unsafe-assignment, @typescript-eslint/no-unsafe-call, @typescript-eslint/no-unsafe-return */
}
