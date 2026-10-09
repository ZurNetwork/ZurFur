import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { denHref } from '$lib/types/brand';

/** Every address the Toggle asked SvelteKit to load, with its options. */
const gone = vi.hoisted(() => [] as { href: string; options: unknown }[]);

vi.mock('$app/navigation', () => ({
	goto: (href: string, options: unknown) => {
		gone.push({ href, options });
		return Promise.resolve();
	}
}));

const { default: ViewOptions } = await import('./ViewOptions.svelte');

const flagLinks = {
	on: denHref('/den/commissions?includeDeleted=true'),
	off: denHref('/den/commissions')
};

describe('ViewOptions', () => {
	it('is a switch reading "Show archived and deactivated", off by default', async () => {
		render(ViewOptions, { includeDeleted: false, flagLinks });
		const toggle = page.getByRole('switch', { name: 'Show archived and deactivated' });
		await expect.element(toggle).toHaveAttribute('aria-checked', 'false');
	});

	it('loads the server-built "on" link in place, keeping focus, when flipped on', async () => {
		gone.length = 0;
		render(ViewOptions, { includeDeleted: false, flagLinks });
		await page.getByRole('switch').click();
		expect(gone).toEqual([
			{ href: '/den/commissions?includeDeleted=true', options: { keepFocus: true, noScroll: true } }
		]);
	});

	it('loads the "off" link when flipped off', async () => {
		gone.length = 0;
		render(ViewOptions, { includeDeleted: true, flagLinks });
		await expect.element(page.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
		await page.getByRole('switch').click();
		expect(gone.map((entry) => entry.href)).toEqual(['/den/commissions']);
	});
});
