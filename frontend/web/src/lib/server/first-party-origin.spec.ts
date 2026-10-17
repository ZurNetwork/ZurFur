import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

/** The two facts the first party is read from: the build, and ZURFUR_PUBLIC_URL. */
interface OriginEnv {
	readonly dev: boolean;
	readonly publicUrl: string | undefined;
}

/** The module's `firstParty`, read fresh at load with `$app/environment` and the env stubbed. */
async function firstPartyUnder(stub: OriginEnv): Promise<string | undefined> {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({
		dev: stub.dev,
		building: false,
		browser: false,
		version: 'test'
	}));
	vi.doMock('$env/dynamic/private', () => ({ env: { ZURFUR_PUBLIC_URL: stub.publicUrl } }));
	const module = await import('./first-party-origin');
	return module.firstParty;
}

/** How long the one cold import may take on a loaded machine. */
const WARM_IMPORT_TIMEOUT_MS = 60_000;

describe('firstParty', () => {
	beforeAll(async () => {
		await import('./first-party-origin');
	}, WARM_IMPORT_TIMEOUT_MS);

	afterEach(() => {
		vi.doUnmock('$app/environment');
		vi.doUnmock('$env/dynamic/private');
		vi.resetModules();
	});

	it('in dev with ZURFUR_PUBLIC_URL unset, is the dev default http://127.0.0.1:8080', async () => {
		await expect(firstPartyUnder({ dev: true, publicUrl: undefined })).resolves.toBe(
			'http://127.0.0.1:8080'
		);
	});

	it('outside dev with ZURFUR_PUBLIC_URL unset, is none', async () => {
		await expect(firstPartyUnder({ dev: false, publicUrl: undefined })).resolves.toBeUndefined();
	});

	it.each([
		[true, 'http://127.0.0.1:9180'],
		[false, 'https://zurfur.app']
	])('with dev %s takes ZURFUR_PUBLIC_URL (%s) when it is set', async (dev, publicUrl) => {
		await expect(firstPartyUnder({ dev, publicUrl })).resolves.toBe(publicUrl);
	});

	it.each(['http://127.0.0.1:8080/?', 'http://127.0.0.1:8080/app', 'not a url', ''])(
		'with a malformed ZURFUR_PUBLIC_URL (%j), is none, even in dev',
		async (malformed) => {
			await expect(firstPartyUnder({ dev: true, publicUrl: malformed })).resolves.toBeUndefined();
			await expect(firstPartyUnder({ dev: false, publicUrl: malformed })).resolves.toBeUndefined();
		}
	);
});
