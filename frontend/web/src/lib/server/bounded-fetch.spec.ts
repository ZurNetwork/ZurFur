import { describe, expect, it } from 'vitest';
import { firstAbortOf, readWhole, settleBy, whileHeld } from './bounded-fetch';

/** What `promise` settled with: its value, or its rejection. */
function settled(promise: Promise<unknown>): Promise<unknown> {
	return promise.then(
		(value) => value,
		(rejected: unknown) => rejected
	);
}

describe('firstAbortOf', () => {
	it('aborts with the reason of whichever signal aborts first', () => {
		const first = new AbortController();
		const second = new AbortController();
		const signal = firstAbortOf([first.signal, second.signal]);

		second.abort(new DOMException('second', 'TimeoutError'));
		first.abort(new DOMException('first', 'TimeoutError'));

		expect(signal.aborted).toBe(true);
		expect((signal.reason as DOMException).message).toBe('second');
	});

	it('is aborted at once when a signal already is', () => {
		const signal = firstAbortOf([AbortSignal.abort(new DOMException('gone', 'TimeoutError'))]);
		expect((signal.reason as DOMException).name).toBe('TimeoutError');
	});
});

describe('settleBy', () => {
	it('fails with the abort’s reason when the signal aborts first, even if the call never ends', async () => {
		const controller = new AbortController();
		const call = settleBy(controller.signal, () => new Promise<string>(() => undefined));

		controller.abort(new DOMException('limit', 'TimeoutError'));

		const failure = await settled(call);
		expect((failure as DOMException).name).toBe('TimeoutError');
	});

	it('hands back what the call answered when it ends first', async () => {
		const controller = new AbortController();
		expect(await settleBy(controller.signal, () => Promise.resolve('answered'))).toBe('answered');
	});

	it('fails at once on a signal that has already aborted, never starting the call', async () => {
		let started = false;
		const failure = await settled(
			settleBy(AbortSignal.abort(new DOMException('gone', 'TimeoutError')), () => {
				started = true;
				return Promise.resolve(1);
			})
		);
		expect((failure as DOMException).name).toBe('TimeoutError');
		expect(started).toBe(false);
	});
	it('fails with what a throwing start threw, leaving no rejection behind for a later abort', async () => {
		const controller = new AbortController();
		const unhandled: unknown[] = [];
		const record = (reason: unknown): void => {
			unhandled.push(reason);
		};
		process.on('unhandledRejection', record);
		try {
			const throwing = (): Promise<string> => {
				throw new Error('refused');
			};
			// Called inside `then`, so a settleBy that threw through would still be measured here.
			const failure = await settled(
				Promise.resolve().then(() => settleBy(controller.signal, throwing))
			);
			controller.abort(new DOMException('limit', 'TimeoutError'));
			await new Promise((turn) => setTimeout(turn, 0));

			expect((failure as Error).message).toBe('refused');
			expect(unhandled).toEqual([]);
		} finally {
			process.off('unhandledRejection', record);
		}
	});
});

describe('readWhole', () => {
	it('keeps the status, the headers and every Set-Cookie, with the body read', async () => {
		const headers = new Headers({ location: '/x' });
		headers.append('set-cookie', 'a=1');
		headers.append('set-cookie', 'b=2');
		const whole = await readWhole(new Response('{"a":1}', { status: 201, headers }));

		expect(whole.status).toBe(201);
		expect(whole.headers.getSetCookie()).toEqual(['a=1', 'b=2']);
		expect(await whole.json()).toEqual({ a: 1 });
	});

	it('keeps a bodiless answer bodiless', async () => {
		const whole = await readWhole(new Response(null, { status: 204 }));
		expect(whole.status).toBe(204);
		expect(whole.body).toBeNull();
	});
});

describe('whileHeld', () => {
	it('hands back what the call answered, and its failure as it was', async () => {
		const request = new Request('http://127.0.0.1:8081/me');
		expect(await whileHeld([request], () => Promise.resolve('answered'))).toBe('answered');
		const failure = await settled(whileHeld([request], () => Promise.reject(new Error('down'))));
		expect((failure as Error).message).toBe('down');
	});
});
