import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Badge from './Badge.svelte';
import type { IconName } from '../icons/icons.ts';
import type { BadgeTone } from './types.ts';
import '../styles/index.ts';

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

describe('Badge', () => {
	it('shows its label as text', async () => {
		render(Badge, { label: 'Open for comms' });
		await expect.element(page.getByText('Open for comms')).toBeInTheDocument();
	});

	it('is neutral by default', () => {
		render(Badge, { label: 'Open', testid: 'badge' });
		const badge = byTestId('badge');
		expect(badge.classList.contains('zf-badge')).toBe(true);
		expect(badge.classList.contains('zf-badge--neutral')).toBe(true);
	});

	it.each(['neutral', 'accent', 'iris', 'crown', 'rose', 'taupe'] as const)(
		'wears the %s tone',
		(tone) => {
			render(Badge, { label: 'Open', tone, testid: 'badge' });
			expect(byTestId('badge').classList.contains(`zf-badge--${tone}`)).toBe(true);
		}
	);

	it.each(['danger', 'Accent', '', 'toString'])('wears neutral for the tone %j', (stranger) => {
		render(Badge, { label: 'Open', tone: stranger as BadgeTone, testid: 'badge' });
		const badge = byTestId('badge');
		expect(badge.classList.contains('zf-badge--neutral')).toBe(true);
		expect([...badge.classList].filter((name) => name.startsWith('zf-badge--'))).toEqual([
			'zf-badge--neutral'
		]);
	});

	it('draws no icon unless given one', () => {
		render(Badge, { label: 'Open', testid: 'badge' });
		expect(byTestId('badge').querySelector('svg')).toBeNull();
	});

	it('draws its icon, hidden from screen readers', () => {
		render(Badge, { label: 'Open', icon: 'clock', testid: 'badge' });
		const icon = byTestId('badge').querySelector('svg');
		expect(icon?.getAttribute('aria-hidden')).toBe('true');
	});

	it('draws no icon the registry does not hold', () => {
		render(Badge, { label: 'Open', icon: 'not-an-icon' as IconName, testid: 'badge' });
		expect(byTestId('badge').querySelector('svg')).toBeNull();
	});

	it('shows the label as text, never markup', () => {
		render(Badge, { label: '<b>Open</b>', testid: 'badge' });
		const badge = byTestId('badge');
		expect(badge.querySelector('b')).toBeNull();
		expect(badge.textContent.trim()).toBe('<b>Open</b>');
	});

	it('shows nothing but its label', () => {
		render(Badge, { label: 'Open', icon: 'clock', testid: 'badge' });
		expect(byTestId('badge').textContent.trim()).toBe('Open');
	});
});
