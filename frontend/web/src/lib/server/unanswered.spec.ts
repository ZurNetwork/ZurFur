import { isHttpError } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import { fetchStub } from '$lib/testing/http';
import { ContractViolation, NetworkFailure } from './api/errors';
import { runRead, UNANSWERED_PAGE, unansweredResponse } from './unanswered';

/** What `promise` rejected with, or `'resolved'`. */
function rejectionOf(promise: Promise<unknown>): Promise<unknown> {
	return promise.then(
		() => 'resolved',
		(rejected: unknown) => rejected
	);
}

const { fetch } = fetchStub(() => Response.json({}));

describe('UNANSWERED_PAGE', () => {
	it('says Zurfur isn’t answering, offers “Try again” on the same address, and runs no script', () => {
		expect(UNANSWERED_PAGE).toContain("<h1>Zurfur isn't answering right now.</h1>");
		expect(UNANSWERED_PAGE).toContain('<a href="">Try again</a>');
		expect(UNANSWERED_PAGE).toContain('<title>Zurfur</title>');
		expect(UNANSWERED_PAGE).not.toMatch(/<script|\son\w+=/i);
	});
});

describe('unansweredResponse', () => {
	it('answers an enhanced post with an error action result, status 503', async () => {
		const request = new Request('http://127.0.0.1:5174/accounts', {
			method: 'POST',
			headers: { 'x-sveltekit-action': 'true', accept: 'application/json' }
		});

		const response = unansweredResponse(request);

		expect(response.status).toBe(503);
		expect(response.headers.get('content-type')).toContain('application/json');
		expect(await response.json()).toEqual({
			type: 'error',
			error: { message: "Zurfur isn't answering right now." }
		});
	});

	it.each([
		['the page', new Request('http://127.0.0.1:5174/accounts')],
		[
			'the action result',
			new Request('http://127.0.0.1:5174/accounts', {
				method: 'POST',
				headers: { 'x-sveltekit-action': 'true' }
			})
		]
	])('marks %s never stored, sending no Referer off the site, and sets no cookie', (_, request) => {
		const response = unansweredResponse(request);

		expect(response.headers.get('cache-control')).toBe('no-store');
		expect(response.headers.get('referrer-policy')).toBe('same-origin');
		expect(response.headers.has('set-cookie')).toBe(false);
	});

	it('answers a GET carrying the action header with the page: only a post is an action', async () => {
		const request = new Request('http://127.0.0.1:5174/accounts', {
			headers: { 'x-sveltekit-action': 'true' }
		});

		expect(await unansweredResponse(request).text()).toBe(UNANSWERED_PAGE);
	});
});

describe('runRead', () => {
	it('hands back what the program read', async () => {
		expect(await runRead(fetch, Effect.succeed('read'))).toBe('read');
	});

	it.each(['unreachable', 'timeout'] as const)(
		'fails the load with 503 when the backend is %s',
		async (reason) => {
			const failure = await rejectionOf(
				runRead(fetch, Effect.fail(new NetworkFailure({ reason })))
			);

			expect(isHttpError(failure, 503)).toBe(true);
		}
	);

	it('leaves any other failure as it was', async () => {
		const violation = new ContractViolation({ path: '/accounts', status: 200, detail: 'x' });

		const failure = await rejectionOf(runRead(fetch, Effect.fail(violation)));

		expect(isHttpError(failure)).toBe(false);
		expect(failure).toBeInstanceOf(Error);
	});
});
