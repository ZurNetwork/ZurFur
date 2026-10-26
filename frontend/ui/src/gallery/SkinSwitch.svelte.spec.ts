import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import SkinSwitch from './SkinSwitch.svelte';
import { SKIN_STORAGE_KEY } from './skin-store.ts';
import { SKIN_ATTRIBUTE } from '../lib/styles/environment.ts';

const root = document.documentElement;

afterEach(() => {
	vi.restoreAllMocks();
	root.removeAttribute(SKIN_ATTRIBUTE);
	localStorage.removeItem(SKIN_STORAGE_KEY);
});

const radio = (name: string) =>
	page.getByRole('group', { name: 'Skin' }).getByRole('radio', { name });

describe('SkinSwitch', () => {
	it('is a radio group named Skin with three choices', async () => {
		render(SkinSwitch);
		const group = page.getByRole('group', { name: 'Skin' });
		await expect.element(group).toBeInTheDocument();
		const names = group
			.getByRole('radio')
			.elements()
			.map((element) => element.closest('label')?.textContent.trim());
		expect(names).toEqual(['Follow the OS', 'Paper dark', 'Paper light']);
	});

	it('starts on Follow the OS with no attribute', async () => {
		render(SkinSwitch);
		await expect.element(radio('Follow the OS')).toBeChecked();
	});

	it('starts from the root attribute', async () => {
		root.setAttribute(SKIN_ATTRIBUTE, 'light');
		render(SkinSwitch);
		await expect.element(radio('Paper light')).toBeChecked();
	});

	it('starts on Follow the OS when the root attribute is not a skin', async () => {
		root.setAttribute(SKIN_ATTRIBUTE, 'LIGHT');
		render(SkinSwitch);
		await expect.element(radio('Follow the OS')).toBeChecked();
	});

	it('wears and stores Paper light, and Follow the OS clears both', async () => {
		const setItem = vi.spyOn(Storage.prototype, 'setItem');
		render(SkinSwitch);
		await radio('Paper light').click();
		expect(root.getAttribute(SKIN_ATTRIBUTE)).toBe('light');
		expect(setItem).toHaveBeenCalledWith(SKIN_STORAGE_KEY, 'light');
		expect(localStorage.getItem(SKIN_STORAGE_KEY)).toBe('light');

		await radio('Follow the OS').click();
		expect(root.hasAttribute(SKIN_ATTRIBUTE)).toBe(false);
		expect(localStorage.getItem(SKIN_STORAGE_KEY)).toBeNull();
	});

	it('moves with the arrow keys', async () => {
		render(SkinSwitch);
		await radio('Follow the OS').click();
		await userEvent.keyboard('{ArrowDown}');
		await expect.element(radio('Paper dark')).toBeChecked();
		expect(root.getAttribute(SKIN_ATTRIBUTE)).toBe('dark');
		expect(localStorage.getItem(SKIN_STORAGE_KEY)).toBe('dark');

		await userEvent.keyboard('{ArrowDown}');
		await expect.element(radio('Paper light')).toBeChecked();
		expect(root.getAttribute(SKIN_ATTRIBUTE)).toBe('light');

		await userEvent.keyboard('{ArrowUp}');
		await expect.element(radio('Paper dark')).toBeChecked();
		expect(root.getAttribute(SKIN_ATTRIBUTE)).toBe('dark');
	});

	it('still wears the skin when storage refuses it', async () => {
		vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new DOMException('full', 'QuotaExceededError');
		});
		render(SkinSwitch);
		await radio('Paper dark').click();
		expect(root.getAttribute(SKIN_ATTRIBUTE)).toBe('dark');
		await expect.element(radio('Paper dark')).toBeChecked();
		expect(localStorage.getItem(SKIN_STORAGE_KEY)).toBeNull();
	});
});
