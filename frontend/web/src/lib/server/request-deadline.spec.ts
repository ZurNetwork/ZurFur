import type { RequestEvent } from '@sveltejs/kit';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { requestDeadline } from './request-deadline';

type HandleInput = Parameters<typeof requestDeadline>[0];

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

/** Run the hook, with `resolve` seeing the event it hands on. */
async function runHook(resolve: (event: RequestEvent) => Promise<Response>) {
	const locals: App.Locals = {};
	const event = { locals } as unknown as RequestEvent;
	const input: HandleInput = { event, resolve };
	const response = await requestDeadline(input);
	return { response, locals };
}

describe('requestDeadline', () => {
	it('hands every hook and load after it one deadline, 15 s away', async () => {
		let deadline: AbortSignal | undefined;
		const pending = runHook(async (event) => {
			deadline = event.locals.deadline;
			await vi.advanceTimersByTimeAsync(14_999);
			const before = deadline?.aborted;
			await vi.advanceTimersByTimeAsync(1);
			return new Response(String(before));
		});

		const { response } = await pending;

		expect(await response.text()).toBe('false');
		expect(deadline?.aborted).toBe(true);
		expect((deadline?.reason as DOMException).name).toBe('TimeoutError');
	});

	it('stops the clock once the response is ready', async () => {
		const { locals } = await runHook(() => Promise.resolve(new Response('page')));

		vi.advanceTimersByTime(60_000);

		expect(locals.deadline?.aborted).toBe(false);
		expect(vi.getTimerCount()).toBe(0);
	});
});
