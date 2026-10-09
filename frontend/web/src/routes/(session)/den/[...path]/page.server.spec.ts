import { isHttpError } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { Effect } from 'effect';
import { fetchStub } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import type { DenPageData } from '$lib/api/den';
import type { Trail } from '$lib/api/trail';

/** Whether this spec's `runApi` answers from the mock Den world instead of the live port. */
const seam = vi.hoisted(() => ({ mock: false, servedLive: false }));

vi.mock('$lib/server/den-served', () => ({ denServed: () => seam.mock || seam.servedLive }));

vi.mock('$lib/server/runtime', async (importActual) => {
	const actual = await importActual<typeof import('$lib/server/runtime')>();
	const { createMockStore, zurfurApiMock } = await import('$lib/server/api/zurfur-api-mock');
	return {
		...actual,
		runApi: <A, E>(
			fetch: typeof globalThis.fetch,
			program: Effect.Effect<A, E, import('$lib/server/api/zurfur-api').ZurfurApi>
		) =>
			seam.mock
				? Effect.runPromise(program.pipe(Effect.provide(zurfurApiMock(createMockStore()))))
				: actual.runApi(fetch, program)
	};
});

const { load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];

/** What the load hands the page. */
interface DenLoadData {
	den: DenPageData;
	trail: Trail;
}

/** Run the load for `address`, as a full page load or an in-app navigation. */
async function runLoad(address: string, isDataRequest = false): Promise<DenLoadData> {
	const url = new URL(address, 'http://127.0.0.1:5174');
	const { fetch } = fetchStub(() => Response.json({}));
	const event = { url, fetch, isDataRequest } as unknown as LoadEvent;
	return (await load(event)) as DenLoadData;
}

describe('/den load on the everyday stack (no Den served)', () => {
	it('is the ordinary 404, with no Den call', async () => {
		seam.mock = false;
		seam.servedLive = false;
		const url = new URL('/den', 'http://127.0.0.1:5174');
		const { fetch, calls } = fetchStub(() => Response.json({}));
		const event = { url, fetch, isDataRequest: false } as unknown as LoadEvent;
		const failure: unknown = await Promise.resolve()
			.then(() => load(event))
			.catch((thrown: unknown) => thrown);
		expect(isHttpError(failure, 404)).toBe(true);
		expect(calls).toEqual([]);
	});
});

describe('/den load on a served but unwired live port', () => {
	it('answers "not connected yet", never an empty Den or the not-found', async () => {
		seam.mock = false;
		seam.servedLive = true;
		const data = await runLoad('/den');
		expect(data.den.outcome).toEqual({ outcome: 'notConnected' });
		seam.servedLive = false;
	});
});

describe('/den load on the mock', () => {
	it('hands the page the Den and the frame its path', async () => {
		seam.mock = true;
		const data = await runLoad('/den/commissions');
		expect(data.den.outcome).toMatchObject({ outcome: 'page' });
		expect(data.trail).toEqual([
			{ step: 'root', href: '/den' },
			{ step: 'named', label: 'commissions', href: undefined }
		]);
	});

	it('reads the folders above the node on a full page load only', async () => {
		seam.mock = true;
		const full = await runLoad('/den/commissions/01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90');
		const navigation = await runLoad('/den/commissions/01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90', true);
		expect(full.den.ancestors.map((listing) => listing.folder)).toEqual([
			'/den',
			'/den/commissions'
		]);
		expect(navigation.den.ancestors).toEqual([]);
	});

	it('answers a refused address with the not-found in the page', async () => {
		seam.mock = true;
		const data = await runLoad('/den/a%2Fb');
		expect(data.den.outcome).toEqual({ outcome: 'notFound' });
	});

	it('redirects a stale page token to the folder’s first page', async () => {
		seam.mock = true;
		const redirect = await expectRedirect(() => runLoad('/den/commissions?pageToken=stale!'));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/den/commissions');
	});
});
