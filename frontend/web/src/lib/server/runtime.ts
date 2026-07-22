/**
 * The runes seam (DD 39944194): the ONE place Effect programs become promises.
 * A single `ManagedRuntime` is built once per server process (module scope —
 * first load/action to import it constructs it); loads, actions and hooks call
 * {@link runApi} and receive plain data. Nothing above this module sees a
 * fiber, and the eslint containment guard keeps `effect` imports below it.
 */

import { Effect, Layer, ManagedRuntime } from 'effect';
import type { FetchFunction } from '$lib/api/client';
import { RequestFetch, ZurfurApi, ZurfurApiLive } from './api/zurfur-api';

/**
 * The process-wide runtime. Holds the request-independent service graph —
 * empty today; config/log/telemetry Layers join here (OTel is a DD follow-up),
 * while per-request services are provided in {@link runApi}.
 */
const runtime = ManagedRuntime.make(Layer.empty);

/**
 * Run an API program for one request: provide the live `ZurfurApi` over the
 * request's own `fetch` (SSR rewrite + cookie forwarding ride inside it), then
 * settle to a promise. Unhandled tagged failures reject and surface as a 500 —
 * `catchTags` the ones a page turns into `redirect()`/`fail()`/data BEFORE
 * the seam, so the error channel documents what's left to blow up.
 */
export function runApi<A, E>(
	fetch: FetchFunction,
	program: Effect.Effect<A, E, ZurfurApi>
): Promise<A> {
	const provided = program.pipe(
		Effect.provide(ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return runtime.runPromise(provided);
}
