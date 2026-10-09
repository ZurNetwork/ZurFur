import { describe, expect, it, vi } from 'vitest';
import { fetchStub, unreachableFetch } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { actions, load } from './+page.server';

type ActionEvent = Parameters<(typeof actions)['default']>[0];

const logoutAction = actions.default;

function logoutEvent(response: () => Response) {
	const deleted: string[] = [];
	const event = {
		fetch: fetchStub(response).fetch,
		cookies: {
			delete: (name: string) => {
				deleted.push(name);
			}
		}
	};
	return { event: event as unknown as ActionEvent, deleted };
}

describe('GET /logout', () => {
	it('is not a page — it just goes home', async () => {
		const bareEvent = {} as Parameters<typeof load>[0];
		const redirect = await expectRedirect(() => load(bareEvent));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/');
	});
});

describe('/logout action', () => {
	it('mirrors the backend cookie clear and lands on a signed-out /', async () => {
		const { event, deleted } = logoutEvent(
			() =>
				new Response(null, {
					status: 303,
					headers: { location: '/', 'set-cookie': 'zurfur.sid=; Max-Age=0; Path=/' }
				})
		);

		const redirect = await expectRedirect(() => logoutAction(event));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/');
		expect(deleted).toEqual(['zurfur.sid']);
	});

	it('clears the browser’s session when the backend answers sign-out with a 500, and says so', async () => {
		const { event, deleted } = logoutEvent(() => new Response('boom', { status: 500 }));

		const redirect = await expectRedirect(() => logoutAction(event));

		expect(deleted).toEqual(['zurfur.sid']);
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/login?signout=unconfirmed');
	});

	it('gives up on a backend that accepts and never answers, still clearing the browser’s session', async () => {
		vi.useFakeTimers();
		const deleted: string[] = [];
		const hanging = ((_input: RequestInfo | URL, init?: RequestInit) =>
			new Promise<Response>((_answer, reject) => {
				init?.signal?.addEventListener('abort', () => {
					reject(new DOMException('aborted', 'AbortError'));
				});
			})) as typeof fetch;
		const event = {
			fetch: hanging,
			cookies: {
				delete: (name: string) => {
					deleted.push(name);
				}
			}
		} as unknown as ActionEvent;

		const settled = expectRedirect(() => logoutAction(event));
		await vi.advanceTimersByTimeAsync(5_000);
		const redirect = await settled;
		vi.useRealTimers();

		expect(deleted).toEqual(['zurfur.sid']);
		expect(redirect.location).toBe('/login?signout=unconfirmed');
	});

	it('clears the browser’s session when the backend is down, and says so', async () => {
		const deleted: string[] = [];
		const event = {
			fetch: unreachableFetch(),
			cookies: {
				delete: (name: string) => {
					deleted.push(name);
				}
			}
		} as unknown as ActionEvent;

		const redirect = await expectRedirect(() => logoutAction(event));

		expect(deleted).toEqual(['zurfur.sid']);
		expect(redirect.location).toBe('/login?signout=unconfirmed');
	});
});
