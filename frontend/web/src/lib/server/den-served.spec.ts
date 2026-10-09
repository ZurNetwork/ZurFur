import { afterEach, describe, expect, it, vi } from 'vitest';

/** `denServed` from a fresh module graph, with mock mode requested or not, in a dev build or not. */
async function denServedWith(options: { mock: boolean; dev: boolean }): Promise<boolean> {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({ building: false, dev: options.dev, browser: false }));
	vi.doMock('$env/dynamic/private', () => ({
		env: options.mock ? { ZURFUR_WEB_MOCK: '1' } : {}
	}));
	const { denServed } = await import('./den-served');
	return denServed();
}

afterEach(() => {
	vi.doUnmock('$app/environment');
	vi.doUnmock('$env/dynamic/private');
	vi.resetModules();
});

describe('denServed', () => {
	it('is false on the everyday stack (the live API)', async () => {
		expect(await denServedWith({ mock: false, dev: true })).toBe(false);
	});

	it('is true on the mock', async () => {
		expect(await denServedWith({ mock: true, dev: true })).toBe(true);
	});

	it('fails closed outside a dev build, even with the mock flag set', async () => {
		expect(await denServedWith({ mock: true, dev: false })).toBe(false);
	});
});
