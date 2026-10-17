import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Checkbox from './Checkbox.svelte';

/** The computed colour a token paints, read through a probe element. */
function tokenColor(token: string): string {
	const probe = document.createElement('span');
	probe.style.color = `var(${token})`;
	document.body.append(probe);
	const color = getComputedStyle(probe).color;
	probe.remove();
	return color;
}

describe('Checkbox', () => {
	it('is named by its label and described by its description', async () => {
		render(Checkbox, { label: 'Include the page', description: '/den/accounts' });

		const box = page.getByRole('checkbox', { name: 'Include the page' });
		await expect.element(box).toBeInTheDocument();
		await expect.element(box).toHaveAccessibleDescription('/den/accounts');
	});

	it('has no description when none is given', async () => {
		render(Checkbox, { label: 'Include the page' });

		await expect
			.element(page.getByRole('checkbox', { name: 'Include the page' }))
			.not.toHaveAttribute('aria-describedby');
	});

	it('ticks from its label', async () => {
		render(Checkbox, { label: 'Include the page', description: 'Not sent' });

		const box = page.getByRole('checkbox', { name: 'Include the page' });
		await expect.element(box).not.toBeChecked();
		await page.getByText('Include the page').click();
		await expect.element(box).toBeChecked();
	});

	it('greys an unticked value with the muted text colour, at full opacity', async () => {
		render(Checkbox, { label: 'Include the page', description: '/den', checked: false });

		await expect.element(page.getByRole('checkbox')).not.toBeChecked();
		const value = page.getByText('/den').element();
		const style = getComputedStyle(value);
		expect(style.color).toBe(tokenColor('--color-text-muted'));
		expect(style.opacity).toBe('1');
	});

	it('shows a ticked value in the text colour', async () => {
		render(Checkbox, { label: 'Include the page', description: '/den', checked: true });

		await expect.element(page.getByRole('checkbox')).toBeChecked();
		const value = page.getByText('/den').element();
		expect(getComputedStyle(value).color).toBe(tokenColor('--color-text'));
	});
});
