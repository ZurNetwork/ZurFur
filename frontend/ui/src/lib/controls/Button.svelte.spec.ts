import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet, type ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import Button from './Button.svelte';
import type { LinkHref } from './link-href.ts';
import type { ButtonVariant } from './types.ts';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** Words for a Button, given as markup already escaped, so the browser reads them as text. */
function words(escaped: string) {
	return createRawSnippet(() => ({ render: () => `<span>${escaped}</span>` }));
}

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

const variants: readonly ButtonVariant[] = ['primary', 'secondary', 'quiet', 'destructive'];

describe('Button', () => {
	it('is a button named by its words, of type button by default', async () => {
		render(Button, { children: words('Send invoice'), onPress: () => undefined });
		const button = page.getByRole('button', { name: 'Send invoice' });
		await expect.element(button).toBeInTheDocument();
		await expect.element(button).toHaveAttribute('type', 'button');
	});

	it('can be a submit button', async () => {
		render(Button, { children: words('Send request'), onPress: () => undefined, type: 'submit' });
		await expect.element(page.getByRole('button')).toHaveAttribute('type', 'submit');
	});

	it('calls onPress with nothing on a click, on Enter and on Space', async () => {
		const onPress = vi.fn<() => void>();
		render(Button, { children: words('Accept'), onPress, testid: 'accept' });
		await page.getByRole('button').click();
		byTestId('accept').focus();
		await userEvent.keyboard('{Enter}');
		await userEvent.keyboard(' ');
		expect(onPress.mock.calls).toEqual([[], [], []]);
	});

	it.each(variants)('presses in the %s look', async (variant) => {
		const onPress = vi.fn<() => void>();
		render(Button, { children: words('Accept'), onPress, variant });
		await page.getByRole('button').click();
		expect(onPress).toHaveBeenCalledTimes(1);
	});

	it.each(variants)(
		'ignores presses while busy in the %s look, and keeps focus',
		async (variant) => {
			const onPress = vi.fn<() => void>();
			render(Button, { children: words('Accept'), onPress, variant, busy: true, testid: 'busy' });
			const button = byTestId('busy');
			expect(button.getAttribute('aria-busy')).toBe('true');
			expect(button.getAttribute('aria-disabled')).toBe('true');
			expect(button.hasAttribute('disabled')).toBe(false);
			button.focus();
			await userEvent.keyboard('{Enter}');
			await userEvent.keyboard(' ');
			await page.getByRole('button').click({ force: true });
			expect(onPress).not.toHaveBeenCalled();
			expect(document.activeElement).toBe(button);
		}
	);

	it('shows one hidden spinner ring beside its words while busy, and records the spin loop on it', () => {
		const calls: SeamCall[] = [];
		render(
			Button,
			{ children: words('Accept'), onPress: () => undefined, busy: true, testid: 'busy' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		const rings = byTestId('busy').querySelectorAll('.zf-spinner');
		expect(rings).toHaveLength(1);
		expect(rings[0]?.getAttribute('aria-hidden')).toBe('true');
		expect(calls.filter((call) => call.kind === 'move' && call.move === 'loop')).toEqual([
			{ kind: 'move', move: 'loop', args: ['spin'], element: rings[0] }
		]);
	});

	it('shows no spinner ring when not busy', () => {
		render(Button, { children: words('Accept'), onPress: () => undefined, testid: 'rest' });
		expect(byTestId('rest').querySelectorAll('.zf-spinner')).toHaveLength(0);
	});

	it('is named by its words alone, busy or not', async () => {
		const { rerender } = render(Button, { children: words('Accept'), onPress: () => undefined });
		await expect
			.element(page.getByRole('button', { name: 'Accept', exact: true }))
			.toBeInTheDocument();
		await rerender({ children: words('Accept'), onPress: () => undefined, busy: true });
		await expect
			.element(page.getByRole('button', { name: 'Accept', exact: true }))
			.toBeInTheDocument();
	});

	it('does not submit its form while busy', () => {
		const onPress = vi.fn<() => void>();
		const { container } = render(Button, {
			children: words('Send request'),
			onPress,
			type: 'submit',
			busy: true,
			testid: 'send'
		});
		const form = document.createElement('form');
		const submitted = vi.fn<(event: SubmitEvent) => void>((event) => {
			event.preventDefault();
		});
		form.addEventListener('submit', submitted);
		container.append(form);
		form.append(byTestId('send'));
		byTestId('send').click();
		expect(submitted).not.toHaveBeenCalled();
		expect(onPress).not.toHaveBeenCalled();
	});

	it.each(variants)('is disabled natively in the %s look and ignores presses', async (variant) => {
		const onPress = vi.fn<() => void>();
		render(Button, { children: words('Accept'), onPress, variant, disabled: true });
		const button = page.getByRole('button');
		await expect.element(button).toBeDisabled();
		await button.click({ force: true });
		expect(onPress).not.toHaveBeenCalled();
	});

	it('has no busy state at rest', () => {
		render(Button, { children: words('Accept'), onPress: () => undefined, testid: 'rest' });
		const button = byTestId('rest');
		expect(button.hasAttribute('aria-busy')).toBe(false);
		expect(button.hasAttribute('aria-disabled')).toBe(false);
	});

	it('is a link when given a link href', async () => {
		const href: LinkHref = '/den/commissions' as LinkHref;
		render(Button, { children: words('Open the queue'), href });
		await expect
			.element(page.getByRole('link', { name: 'Open the queue' }))
			.toHaveAttribute('href', '/den/commissions');
		expect(page.getByRole('button').elements()).toEqual([]);
	});

	it.each(['javascript:alert(1)', '//evil.example', '/a/../b', 'https://x'])(
		'gives a lying link brand %j no href',
		(lie) => {
			render(Button, { children: words('Open'), href: lie as LinkHref, testid: 'lie' });
			const link = byTestId('lie');
			expect(link.tagName).toBe('A');
			expect(link.hasAttribute('href')).toBe(false);
		}
	);

	it('draws an icon before and after its words', () => {
		render(Button, {
			children: words('Send invoice'),
			onPress: () => undefined,
			icon: 'send',
			iconAfter: 'chevron-right',
			testid: 'icons'
		});
		const parts = [...byTestId('icons').children].map((child) => child.tagName.toLowerCase());
		expect(parts).toEqual(['svg', 'span', 'svg']);
	});

	it('draws small icons on a small button', () => {
		render(Button, {
			children: words('Send invoice'),
			onPress: () => undefined,
			icon: 'send',
			size: 'sm',
			testid: 'small'
		});
		const button = byTestId('small');
		expect(button.classList.contains('zf-btn--sm')).toBe(true);
		expect(getComputedStyle(button.querySelector('svg') ?? button).width).toBe('14px');
	});

	it('shows markup in its words as text', async () => {
		const { container } = render(Button, {
			children: words('&lt;img src=x onerror=alert(1)&gt;'),
			onPress: () => undefined
		});
		await expect
			.element(page.getByRole('button', { name: '<img src=x onerror=alert(1)>' }))
			.toBeInTheDocument();
		expect(container.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onPress = vi.fn<() => void>();
		render(Button, { children: words('Accept'), onPress });
		await page.getByRole('button').click();
		expect(onPress).toHaveBeenCalledTimes(1);
	});

	it('squishes when pressed and plays nothing', async () => {
		const calls: SeamCall[] = [];
		render(
			Button,
			{ children: words('Accept'), onPress: () => undefined, testid: 'accept' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		await page.getByRole('button').click();
		expect(calls).toEqual([
			{ kind: 'move', move: 'squish', args: [], element: byTestId('accept') }
		]);
	});

	it('squishes as a link too, and plays nothing', () => {
		const calls: SeamCall[] = [];
		const href: LinkHref = '/den' as LinkHref;
		render(
			Button,
			{ children: words('Open the Den'), href, testid: 'den' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(calls).toEqual([{ kind: 'move', move: 'squish', args: [], element: byTestId('den') }]);
	});

	it.each([
		['primary', 'placed'],
		['secondary', 'placed'],
		['destructive', 'placed'],
		['quiet', 'pick']
	] as const)('lays the %s look as %s', (variant, paper) => {
		render(Button, {
			children: words('Accept'),
			onPress: () => undefined,
			variant,
			testid: 'look'
		});
		const classes = [...byTestId('look').classList].sort();
		expect(classes).toEqual(['zf-btn', `zf-btn--${variant}`, paper].sort());
	});

	it('is secondary by default', () => {
		render(Button, { children: words('Accept'), onPress: () => undefined, testid: 'plain' });
		expect(byTestId('plain').classList.contains('zf-btn--secondary')).toBe(true);
	});
});
