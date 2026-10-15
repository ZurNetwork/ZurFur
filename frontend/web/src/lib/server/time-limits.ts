/**
 * How long the web server waits for the API, in milliseconds: the session
 * check (`/me`), every other call, sign-out, and the whole request (every
 * call one browser request makes, together).
 */
export const TIME_LIMITS = {
	session: 5_000,
	call: 10_000,
	signout: 5_000,
	request: 15_000
} as const;

/** A clock whose signal aborts with a `TimeoutError` once it runs out. */
export interface Countdown {
	readonly signal: AbortSignal;
	/** Stops the clock: the signal then never aborts. */
	readonly stop: () => void;
}

/**
 * A clock that runs out after `ms`. A plain timer rather than
 * `AbortSignal.timeout`, so a call that finishes stops it at once.
 */
export function countdown(ms: number): Countdown {
	const controller = new AbortController();
	const timer = setTimeout(() => {
		controller.abort(new DOMException('The operation timed out.', 'TimeoutError'));
	}, ms);
	const stop = (): void => {
		clearTimeout(timer);
	};
	return { signal: controller.signal, stop };
}
