import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync, tick, type ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import Checkbox from './Checkbox.svelte';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The one checkbox on the page. */
function theBox(): HTMLInputElement {
	const [found] = page.getByRole('checkbox').elements();
	if (!(found instanceof HTMLInputElement)) throw new Error('no checkbox on the page');
	return found;
}

/** The drawn box that sits beside the input. */
function theDrawnBox(container: HTMLElement): HTMLElement {
	const found = container.querySelector('.zf-check__box');
	if (!(found instanceof HTMLElement)) throw new Error('no drawn box');
	return found;
}

/** Whether a part of the drawn box is showing. */
function shows(container: HTMLElement, part: 'tick' | 'dash'): boolean {
	const found = container.querySelector(`.zf-check__${part}`);
	if (found === null) throw new Error(`no ${part} drawn`);
	return getComputedStyle(found).display !== 'none';
}

describe('Checkbox', () => {
	it('is a checkbox named by its label and described by its description', async () => {
		render(Checkbox, {
			label: 'Share sketches with the client',
			description: 'They see each one as soon as you post it',
			checked: false,
			onCheckedChange: () => undefined
		});
		const box = page.getByRole('checkbox', { name: 'Share sketches with the client' });
		await expect.element(box).toBeInTheDocument();
		await expect
			.element(box)
			.toHaveAccessibleDescription('They see each one as soon as you post it');
		await expect.element(box).not.toBeChecked();
	});

	it('has no description when none is given', async () => {
		render(Checkbox, { label: 'Share sketches', checked: true, onCheckedChange: () => undefined });
		await expect.element(page.getByRole('checkbox')).toBeChecked();
		expect(theBox().hasAttribute('aria-describedby')).toBe(false);
	});

	it('hands the parent the value a click asks for, as a plain boolean', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Checkbox, { label: 'Share sketches', checked: false, onCheckedChange });
		await page.getByRole('checkbox').click({ force: true });
		expect(onCheckedChange.mock.calls).toEqual([[true]]);
	});

	it('asks to untick when it is ticked', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Checkbox, { label: 'Share sketches', checked: true, onCheckedChange });
		await page.getByRole('checkbox').click({ force: true });
		expect(onCheckedChange.mock.calls).toEqual([[false]]);
	});

	it('ticks with Space', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Checkbox, { label: 'Share sketches', checked: false, onCheckedChange });
		theBox().focus();
		await userEvent.keyboard(' ');
		expect(onCheckedChange.mock.calls).toEqual([[true]]);
	});

	it('ticks when its label is clicked', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Checkbox, { label: 'Share sketches', checked: false, onCheckedChange });
		await page.getByText('Share sketches').click();
		expect(onCheckedChange.mock.calls).toEqual([[true]]);
	});

	it('ticks when the drawn box is clicked', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		const { container } = render(Checkbox, {
			label: 'Share sketches',
			checked: false,
			onCheckedChange
		});
		await userEvent.click(theDrawnBox(container));
		expect(onCheckedChange.mock.calls).toEqual([[true]]);
	});

	it('stays unticked when the parent refuses the change', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		const { container } = render(Checkbox, {
			label: 'Share sketches',
			checked: false,
			onCheckedChange
		});
		await page.getByText('Share sketches').click();
		flushSync();
		await tick();
		expect(onCheckedChange).toHaveBeenCalledWith(true);
		expect(theBox().checked).toBe(false);
		expect(shows(container, 'tick')).toBe(false);
	});

	it('stays ticked when the parent refuses to untick it', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		const { container } = render(Checkbox, {
			label: 'Share sketches',
			checked: true,
			onCheckedChange
		});
		await page.getByText('Share sketches').click();
		flushSync();
		await tick();
		expect(onCheckedChange).toHaveBeenCalledWith(false);
		expect(theBox().checked).toBe(true);
		expect(shows(container, 'tick')).toBe(true);
	});

	it('stays mixed when the parent refuses the change', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		const { container } = render(Checkbox, {
			label: 'Share every sketch',
			checked: 'mixed',
			onCheckedChange
		});
		await page.getByText('Share every sketch').click();
		flushSync();
		await tick();
		expect(onCheckedChange).toHaveBeenCalledTimes(1);
		expect(theBox().indeterminate).toBe(true);
		expect(theBox().checked).toBe(false);
		expect(shows(container, 'dash')).toBe(true);
		expect(shows(container, 'tick')).toBe(false);
	});

	it('follows the prop when the parent accepts the change', async () => {
		const screen = render(Checkbox, {
			label: 'Share sketches',
			checked: false,
			onCheckedChange: () => undefined
		});
		await screen.rerender({ checked: true });
		expect(theBox().checked).toBe(true);
		await screen.rerender({ checked: 'mixed' });
		expect(theBox().indeterminate).toBe(true);
		await screen.rerender({ checked: false });
		expect(theBox().checked).toBe(false);
		expect(theBox().indeterminate).toBe(false);
	});

	it('shows the tick when ticked and the dash when mixed', () => {
		const ticked = render(Checkbox, {
			label: 'Share sketches',
			checked: true,
			onCheckedChange: () => undefined
		});
		expect(shows(ticked.container, 'tick')).toBe(true);
		expect(shows(ticked.container, 'dash')).toBe(false);
		const mixed = render(Checkbox, {
			label: 'Share every sketch',
			checked: 'mixed',
			onCheckedChange: () => undefined
		});
		expect(shows(mixed.container, 'tick')).toBe(false);
		expect(shows(mixed.container, 'dash')).toBe(true);
		const empty = render(Checkbox, {
			label: 'Share nothing',
			checked: false,
			onCheckedChange: () => undefined
		});
		expect(shows(empty.container, 'tick')).toBe(false);
		expect(shows(empty.container, 'dash')).toBe(false);
	});

	it('asks nothing while disabled', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Checkbox, { label: 'Share sketches', checked: false, onCheckedChange, disabled: true });
		await expect.element(page.getByRole('checkbox')).toBeDisabled();
		await page.getByText('Share sketches').click({ force: true });
		theBox().focus();
		await userEvent.keyboard(' ');
		expect(onCheckedChange).not.toHaveBeenCalled();
	});

	it('renders a name and a value only when given them', () => {
		const plain = render(Checkbox, {
			label: 'Share sketches',
			checked: true,
			onCheckedChange: () => undefined
		});
		const bare = plain.container.querySelector('input');
		expect(bare?.hasAttribute('name')).toBe(false);
		expect(bare?.hasAttribute('value')).toBe(false);
		expect(plain.container.querySelectorAll('input[type="hidden"]')).toHaveLength(0);
		const named = render(Checkbox, {
			label: 'Share every sketch',
			checked: true,
			onCheckedChange: () => undefined,
			name: 'share',
			value: 'sketches'
		});
		const input = named.container.querySelector('input');
		expect([input?.name, input?.getAttribute('value')]).toEqual(['share', 'sketches']);
	});

	it('puts its test id on the checkbox', async () => {
		render(Checkbox, {
			label: 'Share sketches',
			checked: false,
			onCheckedChange: () => undefined,
			testid: 'share-box'
		});
		await expect.element(page.getByTestId('share-box')).toHaveAttribute('type', 'checkbox');
	});

	it('shows markup in its words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		const { container } = render(Checkbox, {
			label: words,
			description: words,
			checked: false,
			onCheckedChange: () => undefined
		});
		await expect.element(page.getByRole('checkbox', { name: words })).toBeInTheDocument();
		expect(container.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Checkbox, { label: 'Share sketches', checked: false, onCheckedChange });
		await page.getByText('Share sketches').click();
		expect(onCheckedChange).toHaveBeenCalledWith(true);
	});

	it('squishes its box and plays the toggle cue on each change, and nothing else', async () => {
		const calls: SeamCall[] = [];
		const { container } = render(
			Checkbox,
			{ label: 'Share sketches', checked: false, onCheckedChange: () => undefined },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(calls).toEqual([
			{ kind: 'move', move: 'squish', args: [], element: theDrawnBox(container) }
		]);
		await page.getByText('Share sketches').click();
		await page.getByText('Share sketches').click();
		expect(calls.slice(1)).toEqual([
			{ kind: 'sound', cue: 'toggle' },
			{ kind: 'sound', cue: 'toggle' }
		]);
	});

	it('shows the one focus ring on the drawn box after the input is focused by key', async () => {
		const { container } = render(Checkbox, {
			label: 'Share sketches',
			checked: false,
			onCheckedChange: () => undefined
		});
		await userEvent.keyboard('{Tab}');
		expect(document.activeElement).toBe(theBox());
		const ring = getComputedStyle(theDrawnBox(container));
		expect(ring.outlineStyle).toBe('solid');
		expect(ring.outlineWidth).toBe('2px');
	});

	it.each([['dark'], ['light']] as const)('takes its colours from the %s skin', (skin) => {
		const scope = document.createElement('div');
		scope.setAttribute('data-skin', skin);
		document.body.append(scope);
		const { container } = render(Checkbox, {
			target: scope,
			props: { label: 'Share sketches', checked: true, onCheckedChange: () => undefined }
		});
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-accent)';
		scope.append(probe);
		expect(getComputedStyle(theDrawnBox(container)).backgroundColor).toBe(
			getComputedStyle(probe).color
		);
		scope.remove();
	});
});
