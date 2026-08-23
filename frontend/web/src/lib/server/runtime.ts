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
import { mockModeEnabled, zurfurApiMock } from './api/zurfur-api-mock';

/**
 * The process-wide runtime. Holds the request-independent service graph —
 * empty today; config/log/telemetry Layers join here (OTel is a DD follow-up),
 * while per-request services are provided in {@link runApi}. `Layer.empty`
 * holds no resources, so it is safe to never dispose; the first resourceful
 * Layer added here must bring `runtime.dispose()` on shutdown and an
 * `import.meta.hot?.dispose` guard for dev HMR.
 */
const runtime = ManagedRuntime.make(Layer.empty);

/**
 * The mock `ZurfurApi` Layer, built ONCE at module scope (ZMVP-198) — every
 * entry `zurfurApiMock()` returns is already LAZY (`Effect.suspend`/
 * `Effect.sync` closures over the shared store, see `zurfur-api-mock.ts`),
 * so state is read at effect-RUN time regardless of how long ago the Layer
 * itself was built; rebuilding it per request (as an earlier revision did)
 * bought nothing. `hooks.server.ts`'s boot-time prod guard is what actually
 * keeps this dead weight outside dev — `mockModeEnabled()` below is always
 * `false` in a real server.
 */
const ZurfurApiMockLive: Layer.Layer<ZurfurApi> = zurfurApiMock();

/**
 * Run an API program for one request: provide `ZurfurApi` — the mock Layer
 * when {@link mockModeEnabled} says `ZURFUR_WEB_MOCK` is live, the real one
 * otherwise (the ONE line that picks) — over the request's own `fetch` (SSR
 * rewrite + cookie forwarding ride inside it for the live Layer; the mock
 * Layer ignores it), then settle to a promise. Unhandled tagged failures
 * reject and surface as a 500 — `catchTags` the ones a page turns into
 * `redirect()`/`fail()`/data BEFORE the seam, so the error channel documents
 * what's left to blow up.
 */
export function runApi<A, E>(
	fetch: FetchFunction,
	program: Effect.Effect<A, E, ZurfurApi>
): Promise<A> {
	const provided = program.pipe(
		Effect.provide(mockModeEnabled() ? ZurfurApiMockLive : ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return runtime.runPromise(provided);
}
