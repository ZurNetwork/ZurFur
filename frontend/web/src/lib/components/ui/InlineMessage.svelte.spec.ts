import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { createRawSnippet } from 'svelte';
import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import InlineMessage from './InlineMessage.svelte';

const words = createRawSnippet(() => ({ render: () => '<span>Report sent.</span>' }));

/** The computed colour a token paints, read through a probe element. */
function tokenColor(token: string): string {
	const probe = document.createElement('span');
	probe.style.color = `var(${token})`;
	document.body.append(probe);
	const color = getComputedStyle(probe).color;
	probe.remove();
	return color;
}

/** The colour of the message's edge. */
function edgeColor(): string {
	const box = page.getByTestId('message').element().firstElementChild;
	if (box === null) throw new Error('the message has no content');
	return getComputedStyle(box).borderLeftColor;
}

describe('InlineMessage', () => {
	it('sits in a status region', async () => {
		render(InlineMessage, { tone: 'info', testid: 'message', children: words });

		const region = page.getByRole('status');
		await expect.element(region).toHaveTextContent('Report sent.');
		await expect.element(page.getByTestId('message')).toHaveAttribute('role', 'status');
	});

	it('keeps the region when it has nothing to say', async () => {
		render(InlineMessage, { tone: 'error', testid: 'message' });

		const region = page.getByRole('status');
		await expect.element(region).toBeInTheDocument();
		await expect.element(region).toHaveTextContent('');
	});

	it('draws the info tone in the accent colour', async () => {
		render(InlineMessage, { tone: 'info', testid: 'message', children: words });

		await expect.element(page.getByRole('status')).toHaveTextContent('Report sent.');
		expect(edgeColor()).toBe(tokenColor('--color-accent'));
	});

	it('draws the error tone in the danger colour', async () => {
		render(InlineMessage, { tone: 'error', testid: 'message', children: words });

		await expect.element(page.getByRole('status')).toHaveTextContent('Report sent.');
		expect(edgeColor()).toBe(tokenColor('--color-danger'));
	});
});
