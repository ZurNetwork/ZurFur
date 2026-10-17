import { beforeEach, describe, expect, it, vi } from 'vitest';
import { expectRedirect } from '$lib/testing/redirect';

/** The run-time env the layout reads the build id from. */
const env = vi.hoisted((): Record<string, string | undefined> => ({}));
vi.mock('$env/dynamic/private', () => ({ env }));

const { load } = await import('./+layout.server');

type LoadEvent = Parameters<typeof load>[0];

function guardEvent(session: unknown): LoadEvent {
	return { parent: () => Promise.resolve({ session }) } as unknown as LoadEvent;
}

const signedIn = {
	did: 'did:plc:alice',
	handle: undefined,
	displayName: undefined,
	avatarUrl: undefined
};

beforeEach(() => {
	delete env.ZURFUR_BUILD_ID;
});

describe('(session) guard', () => {
	it('bounces an anonymous visit to /login', async () => {
		const redirect = await expectRedirect(() => load(guardEvent(undefined)));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/login');
	});

	it('passes a signed-in visit through, handing the frame its session', async () => {
		env.ZURFUR_BUILD_ID = 'build-7';
		await expect(load(guardEvent(signedIn))).resolves.toStrictEqual({
			session: signedIn,
			buildId: 'build-7'
		});
	});
});

describe('(session) build id', () => {
	it('shows "unknown" when ZURFUR_BUILD_ID is unset', async () => {
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ buildId: 'unknown' });
	});

	it.each(['', '   ', 'x'.repeat(65), `build${String.fromCodePoint(0x202e)}`])(
		'shows "unknown" for a ZURFUR_BUILD_ID that does not fit (%j)',
		async (raw) => {
			env.ZURFUR_BUILD_ID = raw;
			await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ buildId: 'unknown' });
		}
	);

	it('reads ZURFUR_BUILD_ID at each load, trimmed', async () => {
		env.ZURFUR_BUILD_ID = ' build-8 ';
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ buildId: 'build-8' });
	});
});
