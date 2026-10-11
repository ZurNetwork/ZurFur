import { isHttpError } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { errorCopy } from '$lib/components/explorer/error-copy';
import { fetchStub } from '$lib/testing/http';
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

	it('fails loudly when the backend answers sign-out with a 500', async () => {
		const { event, deleted } = logoutEvent(() => new Response('boom', { status: 500 }));
		await expect(logoutAction(event)).rejects.toMatchObject({ status: 502 });
		expect(deleted).toEqual([]);
	});

	it('carries no message of its own: the error page picks the words from the route', async () => {
		const { event } = logoutEvent(() => new Response('boom', { status: 500 }));
		const failure: unknown = await logoutAction(event).catch((thrown: unknown) => thrown);
		expect(isHttpError(failure, 502)).toBe(true);
		if (isHttpError(failure)) expect(failure.body.message).not.toContain('Sign-out');
		expect(errorCopy(502, '/(session)/logout').message).toBe(
			'Sign-out did not complete. Try again.'
		);
	});
});
