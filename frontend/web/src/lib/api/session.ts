/**
 * Typed calls onto the backend's browser-session surface (`session.rs`):
 * whoami and the sign-in start. Session mechanics (`zurfur.sid`, expiry,
 * CSRF) live entirely backend-side — these helpers only consume them.
 */

import { apiFetch } from './client';
import { isProblem, PROBLEM_CONTENT_TYPE, type Problem } from './problem';

/**
 * The JSON `/me` contract (ZMVP-151 slice 1): `did` always present for a
 * live session; the profile fields are `null` when the PDS was unreachable
 * and nothing was cached — callers fall back to showing the DID.
 */
export interface Session {
	did: string;
	handle: string | null;
	display_name: string | null;
	avatar_url: string | null;
}

/**
 * Who is signed in, if anyone. `null` on the backend's 401
 * `not_authenticated` (anonymous or expired); any other problem is
 * unexpected on this endpoint and throws.
 */
export async function getSession(fetch: typeof globalThis.fetch): Promise<Session | null> {
	const result = await apiFetch<Session>(fetch, '/me');
	if (result.ok) return result.data;
	if (result.problem.code === 'not_authenticated') return null;
	throw new Error(`unexpected problem from /me: ${result.problem.code}`);
}

/**
 * The catch-arm for session lookups that degrade to anonymous instead of
 * failing the page: `null` only when the backend was unreachable (`fetch`
 * rejects with a `TypeError`). A contract violation or unexpected problem
 * still throws — a regression must not masquerade as "signed out".
 */
export function anonymousWhenUnreachable(error: unknown): null {
	if (error instanceof TypeError) return null;
	throw error;
}

/** The two ways a sign-in start comes back: bounce to the PDS, or a problem to render. */
export type SigninStart = { location: string } | { problem: Problem };

/**
 * Start the atproto OAuth flow for `handle` via `POST /signin`. Server-action
 * only: it reads the 303 `Location` (the PDS authorize URL) off the response,
 * which needs `redirect: 'manual'` semantics only server-side fetch provides
 * (a browser fetch would return an opaque redirect with no headers).
 */
export async function startSignin(
	fetch: typeof globalThis.fetch,
	handle: string
): Promise<SigninStart> {
	const form = new URLSearchParams({ handle });
	const response = await fetch('/api/signin', {
		method: 'POST',
		body: form,
		redirect: 'manual'
	});

	if (response.status >= 300 && response.status < 400) {
		const location = response.headers.get('location');
		if (location === null) {
			throw new Error('signin redirect carried no Location header');
		}
		return { location };
	}

	const contentType = response.headers.get('content-type') ?? '';
	if (contentType.startsWith(PROBLEM_CONTENT_TYPE)) {
		const body: unknown = await response.json().catch(() => null);
		if (isProblem(body)) return { problem: body };
	}
	throw new Error(
		`API contract violation: /signin responded ${response.status} without a problem body`
	);
}
