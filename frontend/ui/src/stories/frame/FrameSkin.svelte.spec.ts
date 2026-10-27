/*
 * FrameSkin against the OS colour scheme: this spec emulates media features,
 * so it runs in the client-media project, alone, after the others.
 */
import { commands } from 'vitest/browser';
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import FrameSkin from './FrameSkin.svelte';
import type { SkinChoice } from './globals.ts';
import type { Skin } from '../../lib/styles/environment.ts';

/** Content that prints the skin it was handed. */
const children = createRawSnippet((skin: () => Skin) => ({
	render: () => `<span class="shown">${skin()}</span>`,
	setup: (element) => {
		$effect(() => {
			element.textContent = skin();
		});
	}
}));

afterEach(async () => {
	expect(await commands.emulateMedia({ colorScheme: null })).toEqual({ kind: 'applied' });
});

/** Emulates the OS colour scheme and waits until the page sees it. */
async function emulateScheme(scheme: Skin): Promise<void> {
	expect(await commands.emulateMedia({ colorScheme: scheme })).toEqual({ kind: 'applied' });
	await expect.poll(() => matchMedia(`(prefers-color-scheme: ${scheme})`).matches).toBe(true);
}

function shown(): string | undefined {
	return document.querySelector('.shown')?.textContent;
}

function frameSkin(choice: SkinChoice) {
	return render(FrameSkin, { choice, children });
}

describe('FrameSkin', () => {
	it.each([
		['dark', 'light'],
		['light', 'dark']
	] as const)('shows %s as is, whatever the OS asks (here %s)', async (choice, scheme) => {
		await emulateScheme(scheme);
		frameSkin(choice);
		expect(shown()).toBe(choice);
	});

	it.each(['dark', 'light'] as const)('follows the OS when it asks for %s', async (scheme) => {
		await emulateScheme(scheme);
		frameSkin('os');
		expect(shown()).toBe(scheme);
	});

	it('follows the OS live when it changes', async () => {
		await emulateScheme('dark');
		frameSkin('os');
		expect(shown()).toBe('dark');
		await emulateScheme('light');
		await expect.poll(shown).toBe('light');
		await emulateScheme('dark');
		await expect.poll(shown).toBe('dark');
	});

	it('moves from a picked skin back to the OS', async () => {
		await emulateScheme('light');
		const screen = frameSkin('dark');
		expect(shown()).toBe('dark');
		await screen.rerender({ choice: 'os' });
		await expect.poll(shown).toBe('light');
	});
});
