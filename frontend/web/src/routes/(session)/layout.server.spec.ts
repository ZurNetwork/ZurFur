import { beforeEach, describe, expect, it, vi } from 'vitest';
import { expectRedirect } from '$lib/testing/redirect';

/** The run-time env the layout reads the build id, the stack and mock mode from. */
const env = vi.hoisted((): Record<string, string | undefined> => ({}));
vi.mock('$env/dynamic/private', () => ({ env }));

/** Whether this is a dev build, as `$app/environment` says, read at each load. */
const build = vi.hoisted(() => ({ dev: true }));
vi.mock('$app/environment', () => ({
	get dev() {
		return build.dev;
	},
	building: false,
	browser: false,
	version: 'test'
}));

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
	delete env.ZURFUR_WEB_STACK;
	delete env.ZURFUR_WEB_MOCK;
	build.dev = true;
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
			buildId: 'build-7',
			testStack: 'none'
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

describe('(session) the stack the report form names', () => {
	it('is the dev profile’s test stack when ZURFUR_WEB_STACK=dev-profile in a dev build', async () => {
		env.ZURFUR_WEB_STACK = 'dev-profile';
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'devProfile' });
	});

	it('is a real one when ZURFUR_WEB_STACK is unset (`just dev`)', async () => {
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'none' });
	});

	it.each(['', 'dev', 'Dev-Profile', ' dev-profile', 'production'])(
		'is a real one for any other ZURFUR_WEB_STACK (%j)',
		async (raw) => {
			env.ZURFUR_WEB_STACK = raw;
			await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'none' });
		}
	);

	it('is never the test stack outside a dev build', async () => {
		build.dev = false;
		env.ZURFUR_WEB_STACK = 'dev-profile';
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'none' });
	});

	it('is mock mode when ZURFUR_WEB_MOCK=1 in a dev build, even on the dev profile', async () => {
		env.ZURFUR_WEB_MOCK = '1';
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'mock' });
		env.ZURFUR_WEB_STACK = 'dev-profile';
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'mock' });
	});

	it('is never mock mode outside a dev build', async () => {
		build.dev = false;
		env.ZURFUR_WEB_MOCK = '1';
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'none' });
	});

	it('reads the variables at each load', async () => {
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'none' });
		env.ZURFUR_WEB_STACK = 'dev-profile';
		await expect(load(guardEvent(signedIn))).resolves.toMatchObject({ testStack: 'devProfile' });
	});
});
