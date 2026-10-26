import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import IconButton from './IconButton.svelte';
import type { IconButtonVariant } from './types.ts';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** The tooltip's words on the page, wherever it portalled, or undefined while it is hidden. */
function shownTip(): HTMLElement | undefined {
	const tip = document.querySelector('.zf-tip');
	return tip instanceof HTMLElement ? tip : undefined;
}

const variants: readonly IconButtonVariant[] = ['quiet', 'secondary'];

describe('IconButton', () => {
	it('is a button named by its label, with no description', async () => {
		render(IconButton, { icon: 'copy', label: 'Copy path', onPress: () => undefined });
		const button = page.getByRole('button', { name: 'Copy path' });
		await expect.element(button).toBeInTheDocument();
		await expect.element(button).toHaveAccessibleDescription('');
		await expect.element(button).toHaveAttribute('type', 'button');
	});

	it('shows its label as a tip on focus, adding no second description', async () => {
		render(IconButton, {
			icon: 'copy',
			label: 'Copy path',
			onPress: () => undefined,
			testid: 'copy'
		});
		byTestId('copy').focus();
		await vi.waitFor(() => {
			expect(shownTip()?.textContent).toBe('Copy path');
		});
		const button = page.getByRole('button', { name: 'Copy path' });
		await expect.element(button).toHaveAccessibleDescription('');
		expect(byTestId('copy').hasAttribute('aria-describedby')).toBe(false);
		expect(shownTip()?.getAttribute('aria-hidden')).toBe('true');
	});

	it('shows its tip on hover', async () => {
		render(IconButton, { icon: 'copy', label: 'Copy path', onPress: () => undefined });
		await page.getByRole('button').hover();
		await vi.waitFor(
			() => {
				expect(shownTip()?.textContent).toBe('Copy path');
			},
			{ timeout: 3000 }
		);
		await expect.element(page.getByRole('button')).toHaveAccessibleDescription('');
	});

	it('hides its tip on Escape and keeps focus', async () => {
		render(IconButton, {
			icon: 'copy',
			label: 'Copy path',
			onPress: () => undefined,
			testid: 'copy'
		});
		byTestId('copy').focus();
		await vi.waitFor(() => {
			expect(shownTip()).toBeDefined();
		});
		await userEvent.keyboard('{Escape}');
		await vi.waitFor(() => {
			expect(shownTip()).toBeUndefined();
		});
		expect(document.activeElement).toBe(byTestId('copy'));
	});

	it('calls onPress with nothing on a click, on Enter and on Space', async () => {
		const onPress = vi.fn<() => void>();
		render(IconButton, { icon: 'copy', label: 'Copy path', onPress, testid: 'copy' });
		await page.getByRole('button').click();
		byTestId('copy').focus();
		await userEvent.keyboard('{Enter}');
		await userEvent.keyboard(' ');
		expect(onPress.mock.calls).toEqual([[], [], []]);
	});

	it.each(variants)('presses in the %s look', async (variant) => {
		const onPress = vi.fn<() => void>();
		render(IconButton, { icon: 'copy', label: 'Copy path', onPress, variant });
		await page.getByRole('button').click();
		expect(onPress).toHaveBeenCalledTimes(1);
	});

	it.each(variants)('is disabled natively in the %s look and ignores presses', async (variant) => {
		const onPress = vi.fn<() => void>();
		render(IconButton, { icon: 'copy', label: 'Copy path', onPress, variant, disabled: true });
		const button = page.getByRole('button');
		await expect.element(button).toBeDisabled();
		await button.click({ force: true });
		expect(onPress).not.toHaveBeenCalled();
	});

	it('says pressed, expanded and what it controls only when told', async () => {
		render(IconButton, {
			icon: 'eye',
			label: 'Show hidden files',
			onPress: () => undefined,
			pressed: true,
			expanded: false,
			controls: 'file-list',
			testid: 'tool'
		});
		const button = page.getByRole('button', { name: 'Show hidden files', pressed: true });
		await expect.element(button).toHaveAttribute('aria-expanded', 'false');
		await expect.element(button).toHaveAttribute('aria-controls', 'file-list');
		render(IconButton, {
			icon: 'copy',
			label: 'Copy path',
			onPress: () => undefined,
			testid: 'plain'
		});
		const attributes = ['aria-pressed', 'aria-expanded', 'aria-controls'];
		expect(attributes.filter((name) => byTestId('plain').hasAttribute(name))).toEqual([]);
	});

	it('shows markup in its label as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		const { container } = render(IconButton, {
			icon: 'copy',
			label: words,
			onPress: () => undefined,
			testid: 'markup'
		});
		await expect.element(page.getByRole('button', { name: words })).toBeInTheDocument();
		byTestId('markup').focus();
		await vi.waitFor(() => {
			expect(shownTip()?.textContent).toBe(words);
		});
		expect(container.querySelector('img')).toBeNull();
		expect(document.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onPress = vi.fn<() => void>();
		render(IconButton, { icon: 'copy', label: 'Copy path', onPress });
		await page.getByRole('button').click();
		expect(onPress).toHaveBeenCalledTimes(1);
	});

	it('squishes once, through its tip showing and hiding, and plays nothing', async () => {
		const calls: SeamCall[] = [];
		render(
			IconButton,
			{ icon: 'copy', label: 'Copy path', onPress: () => undefined, testid: 'copy' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		await page.getByRole('button').click();
		byTestId('copy').blur();
		byTestId('copy').focus();
		await vi.waitFor(() => {
			expect(shownTip()).toBeDefined();
		});
		await userEvent.keyboard('{Escape}');
		await vi.waitFor(() => {
			expect(shownTip()).toBeUndefined();
		});
		expect(calls).toEqual([{ kind: 'move', move: 'squish', args: [], element: byTestId('copy') }]);
	});

	it.each([
		['quiet', 'pick'],
		['secondary', 'placed']
	] as const)('lays the %s look as %s', (variant, paper) => {
		render(IconButton, {
			icon: 'copy',
			label: 'Copy path',
			onPress: () => undefined,
			variant,
			testid: 'look'
		});
		const classes = [...byTestId('look').classList].sort();
		expect(classes).toEqual(['zf-iconbtn', `zf-iconbtn--${variant}`, paper].sort());
	});

	it('is quiet by default, and small when asked', () => {
		render(IconButton, {
			icon: 'copy',
			label: 'Copy path',
			onPress: () => undefined,
			size: 'sm',
			testid: 'small'
		});
		const button = byTestId('small');
		expect(button.classList.contains('zf-iconbtn--quiet')).toBe(true);
		expect(button.classList.contains('zf-iconbtn--sm')).toBe(true);
		expect(getComputedStyle(button).width).toBe('28px');
		expect(getComputedStyle(button.querySelector('svg') ?? button).width).toBe('14px');
	});
});
