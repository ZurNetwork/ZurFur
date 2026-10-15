import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { countdown, TIME_LIMITS } from './time-limits';

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('TIME_LIMITS', () => {
	it('waits 5 s for the session check and sign-out, 10 s for any other call, 15 s for a request', () => {
		expect(TIME_LIMITS).toEqual({ session: 5_000, call: 10_000, signout: 5_000, request: 15_000 });
	});
});

describe('countdown', () => {
	it('aborts its signal with a TimeoutError once it runs out, and not before', () => {
		const clock = countdown(1_000);

		vi.advanceTimersByTime(999);
		expect(clock.signal.aborted).toBe(false);
		vi.advanceTimersByTime(1);

		expect(clock.signal.aborted).toBe(true);
		expect(clock.signal.reason).toBeInstanceOf(DOMException);
		expect((clock.signal.reason as DOMException).name).toBe('TimeoutError');
	});

	it('never aborts once stopped, and leaves no timer behind', () => {
		const clock = countdown(1_000);

		clock.stop();
		vi.advanceTimersByTime(5_000);

		expect(clock.signal.aborted).toBe(false);
		expect(vi.getTimerCount()).toBe(0);
	});
});
