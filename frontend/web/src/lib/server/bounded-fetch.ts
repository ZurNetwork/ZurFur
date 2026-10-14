/**
 * The pieces of a bounded backend call: it settles by its signal on its own
 * timer, and the abort travels through listeners and held Requests only.
 */

/** The error a call stopped by `signal` fails with: the abort's reason, or an `AbortError`. */
function abortReason(signal: AbortSignal): Error {
	const reason: unknown = signal.reason;
	if (reason instanceof Error) return reason;
	return new DOMException('The operation was aborted.', 'AbortError');
}

/**
 * A signal that aborts, with the same reason, as soon as the first of
 * `signals` does. Its controller is held by the listeners on those signals,
 * so it stays reachable as long as any of them is.
 */
export function firstAbortOf(signals: readonly AbortSignal[]): AbortSignal {
	const controller = new AbortController();
	const already = signals.find((signal) => signal.aborted);
	if (already !== undefined) {
		controller.abort(abortReason(already));
		return controller.signal;
	}
	for (const signal of signals) {
		signal.addEventListener(
			'abort',
			() => {
				controller.abort(abortReason(signal));
			},
			{ once: true }
		);
	}
	return controller.signal;
}

/**
 * The call `start` makes, settled no later than `signal` aborts: from then on
 * it fails with the abort's reason, even if the abort never reaches the call
 * itself. Once the signal has aborted, the call is never started; a `start`
 * that throws fails it with what was thrown.
 */
export function settleBy<T>(signal: AbortSignal, start: () => Promise<T>): Promise<T> {
	if (signal.aborted) return Promise.reject(abortReason(signal));
	let stop = (): void => undefined;
	const aborted = new Promise<never>((_settle, fail) => {
		const onAbort = (): void => {
			fail(abortReason(signal));
		};
		signal.addEventListener('abort', onAbort, { once: true });
		stop = () => {
			signal.removeEventListener('abort', onAbort);
		};
	});
	// A `start` that throws rejects `call` instead, so the race still holds the abort's rejection.
	const call = new Promise<T>((answer) => {
		answer(start());
	});
	return Promise.race([call, aborted]).finally(stop);
}

/** `response` with its body read into memory, so nothing more is read from the network. */
export async function readWhole(response: Response): Promise<Response> {
	const stream = response.body ?? undefined;
	const body = stream === undefined ? undefined : await response.arrayBuffer();
	return new Response(body, response);
}

/** The requests whose calls are still running, held so none is collected mid-call. */
const IN_FLIGHT = new Set<Request>();

/** Runs `call` with `requests` held until it settles, so each keeps passing its signal's abort on. */
export async function whileHeld<T>(
	requests: readonly Request[],
	call: () => Promise<T>
): Promise<T> {
	for (const request of requests) IN_FLIGHT.add(request);
	try {
		return await call();
	} finally {
		for (const request of requests) IN_FLIGHT.delete(request);
	}
}
