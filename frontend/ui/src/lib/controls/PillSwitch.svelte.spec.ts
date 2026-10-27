import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import AcceptingPillSwitch from '../../checks/testing/AcceptingPillSwitch.svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import type { PillOption } from './types';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

type Batch = 'all' | 'spooky_sketches' | 'stickers';

const mixed: readonly PillOption<Batch>[] = [
	{ id: 'all', label: 'All' },
	{ id: 'spooky_sketches', label: 'spooky_sketches', prefix: 'batch:' },
	{ id: 'stickers', label: 'stickers', prefix: 'phase:' }
];

const batches: readonly PillOption<Batch>[] = [
	{ id: 'all', label: 'All' },
	{ id: 'spooky_sketches', label: 'spooky_sketches', prefix: 'batch:' },
	{ id: 'stickers', label: 'stickers', prefix: 'batch:' }
];

interface Spec {
	readonly options?: readonly PillOption<Batch>[];
	readonly initial?: Batch;
	readonly accepts?: boolean;
	readonly disabled?: boolean;
	readonly name?: string;
	readonly testid?: string;
	readonly label?: string;
}

/** Renders the switch under a parent that takes (or refuses) what it is told; returns the spy. */
function renderSwitch(
	spec: Spec = {},
	wrapped?: (call: SeamCall) => void,
	target?: HTMLElement
): ReturnType<typeof vi.fn<(id: Batch) => void>> {
	const onChange = vi.fn<(id: Batch) => void>();
	const props = {
		label: spec.label ?? 'Batch',
		options: spec.options ?? batches,
		initial: spec.initial ?? 'all',
		accepts: spec.accepts ?? true,
		disabled: spec.disabled ?? false,
		name: spec.name,
		testid: spec.testid,
		onChange
	};
	const options = target === undefined ? { props } : { target, props };
	if (wrapped === undefined) {
		render(AcceptingPillSwitch<Batch>, options);
	} else {
		const recording = { wrapper: RecordingEnvironment, wrapperProps: { onRecord: wrapped } };
		render(AcceptingPillSwitch<Batch>, options, recording);
	}
	return onChange;
}

/** The radio with this accessible name. */
function radio(name: string): HTMLElement {
	const [found] = page.getByRole('radio', { name }).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`no radio named ${name}`);
	return found;
}

afterEach(() => {
	for (const copy of document.querySelectorAll('[data-copy]')) copy.remove();
});

describe('PillSwitch', () => {
	it('is a radio group named by its label, with a radio for each option', async () => {
		renderSwitch();
		const group = page.getByRole('radiogroup', { name: 'Batch' });
		await expect.element(group).toHaveAttribute('data-orientation', 'horizontal');
		expect(
			page
				.getByRole('radio')
				.elements()
				.map((found) => found.textContent.trim())
		).toEqual(['All', 'spooky_sketches', 'stickers']);
		await expect
			.element(page.getByRole('radio', { name: 'All' }))
			.toHaveAttribute('aria-checked', 'true');
		await expect
			.element(page.getByRole('radio', { name: 'stickers' }))
			.toHaveAttribute('aria-checked', 'false');
	});

	it('shows a type every typed option shares once, beside the switch, and the options drop it', () => {
		renderSwitch();
		const types = [...document.querySelectorAll('.zf-seg__type')];
		expect(types.map((found) => found.textContent)).toEqual(['batch:']);
		const group = page.getByRole('radiogroup', { name: 'Batch' }).element();
		const [type] = types;
		expect(type?.id).not.toBe('');
		expect(group.getAttribute('aria-describedby')).toBe(type?.id);
		expect(group.previousElementSibling).toBe(type);
		expect(group.parentElement?.tagName).toBe('DIV');
		expect(group.parentElement?.classList.contains('zf-seg-row')).toBe(true);
		expect(document.querySelectorAll('.zf-seg__prefix')).toHaveLength(0);
		expect(radio('stickers').classList.contains('zf-seg__opt--typed')).toBe(true);
		expect(radio('All').classList.contains('zf-seg__opt--typed')).toBe(false);
	});

	it('keeps each option’s own muted type before its word when the types differ', () => {
		renderSwitch({ options: mixed });
		expect(document.querySelectorAll('.zf-seg__type')).toHaveLength(0);
		const group = page.getByRole('radiogroup', { name: 'Batch' }).element();
		expect(group.hasAttribute('aria-describedby')).toBe(false);
		expect(
			page
				.getByRole('radio')
				.elements()
				.map((found) => found.textContent.trim())
		).toEqual(['All', 'batch: spooky_sketches', 'phase: stickers']);
		const typed = radio('stickers');
		expect(typed.classList.contains('zf-seg__opt--typed')).toBe(true);
		expect(typed.querySelector('.zf-seg__prefix')?.textContent).toBe('phase:');
		expect(radio('All').querySelector('.zf-seg__prefix')).toBeNull();
	});

	it('shows no type when no option is typed, or the type is empty', () => {
		const empty: readonly PillOption<Batch>[] = [
			{ id: 'all', label: 'All', prefix: '' },
			{ id: 'stickers', label: 'stickers' }
		];
		renderSwitch({ options: empty });
		expect(document.querySelectorAll('.zf-seg__type, .zf-seg__prefix')).toHaveLength(0);
		const group = page.getByRole('radiogroup', { name: 'Batch' }).element();
		expect(group.hasAttribute('aria-describedby')).toBe(false);
		expect(radio('All').classList.contains('zf-seg__opt--typed')).toBe(false);
	});

	it('keeps the same DOM around the group when there is no shared type', () => {
		const screen = renderSwitch({ options: mixed, testid: 'batch-filter' });
		void screen;
		const group = page.getByTestId('batch-filter').element();
		expect(group.parentElement?.classList.contains('zf-seg-row')).toBe(false);
	});

	it('hands the parent the id a click asks for, as the typed id', async () => {
		const onChange = renderSwitch();
		await page.getByRole('radio', { name: 'stickers' }).click();
		expect(onChange.mock.calls).toEqual([['stickers']]);
		await expect
			.element(page.getByRole('radio', { name: 'stickers' }))
			.toHaveAttribute('aria-checked', 'true');
	});

	it('moves focus and chooses with the right arrow, and follows when the parent accepts', async () => {
		const onChange = renderSwitch();
		radio('All').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onChange.mock.calls).toEqual([['spooky_sketches']]);
		expect(document.activeElement).toBe(radio('spooky_sketches'));
		await expect
			.element(page.getByRole('radio', { name: 'spooky_sketches' }))
			.toHaveAttribute('aria-checked', 'true');
		await expect
			.element(page.getByRole('radio', { name: 'All' }))
			.toHaveAttribute('aria-checked', 'false');
	});

	it('moves focus and chooses with the left arrow, wrapping round the ends', async () => {
		const onChange = renderSwitch();
		radio('All').focus();
		await userEvent.keyboard('{ArrowLeft}');
		expect(onChange.mock.calls).toEqual([['stickers']]);
		expect(document.activeElement).toBe(radio('stickers'));
	});

	it('moves and chooses with the down and up arrows too', async () => {
		const onChange = renderSwitch();
		radio('All').focus();
		await userEvent.keyboard('{ArrowDown}');
		await userEvent.keyboard('{ArrowUp}');
		expect(onChange.mock.calls).toEqual([['spooky_sketches'], ['all']]);
	});

	it('moves and chooses the first and last option with Home and End', async () => {
		const onChange = renderSwitch({ initial: 'spooky_sketches' });
		radio('spooky_sketches').focus();
		await userEvent.keyboard('{End}');
		await userEvent.keyboard('{Home}');
		expect(onChange.mock.calls).toEqual([['stickers'], ['all']]);
	});

	it('chooses the focused option with Space', async () => {
		const onChange = renderSwitch();
		radio('stickers').focus();
		await userEvent.keyboard(' ');
		expect(onChange.mock.calls).toEqual([['stickers']]);
		expect(document.activeElement).toBe(radio('stickers'));
	});

	it('takes focus on the chosen option alone when tabbed to', () => {
		renderSwitch({ initial: 'spooky_sketches' });
		const tabStops = page
			.getByRole('radio')
			.elements()
			.filter((found) => found.getAttribute('tabindex') === '0');
		expect(tabStops).toEqual([radio('spooky_sketches')]);
	});

	it('stays on the prop’s value when the parent refuses, though the arrow still asks', async () => {
		const onChange = renderSwitch({ accepts: false });
		radio('All').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onChange.mock.calls).toEqual([['spooky_sketches']]);
		await expect
			.element(page.getByRole('radio', { name: 'All' }))
			.toHaveAttribute('aria-checked', 'true');
		await expect
			.element(page.getByRole('radio', { name: 'spooky_sketches' }))
			.toHaveAttribute('aria-checked', 'false');
	});

	it('stays on the prop’s value when the parent refuses a click', async () => {
		const onChange = renderSwitch({ accepts: false });
		await page.getByRole('radio', { name: 'stickers' }).click();
		expect(onChange.mock.calls).toEqual([['stickers']]);
		await expect
			.element(page.getByRole('radio', { name: 'All' }))
			.toHaveAttribute('aria-checked', 'true');
	});

	it('asks nothing for an option whose id is empty', async () => {
		const options: readonly PillOption<Batch>[] = [
			{ id: 'all', label: 'All' },
			// An id the DOM could hand back as "": the lookup must refuse it, never send it on.
			{ id: '' as Batch, label: 'None' }
		];
		const onChange = renderSwitch({ options });
		await page.getByRole('radio', { name: 'None' }).click();
		radio('All').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onChange).not.toHaveBeenCalled();
	});

	it('cannot choose a disabled option, by click or by arrow, and the arrows skip it', async () => {
		const options: readonly PillOption<Batch>[] = [
			{ id: 'all', label: 'All' },
			{ id: 'spooky_sketches', label: 'spooky_sketches', prefix: 'batch:', disabled: true },
			{ id: 'stickers', label: 'stickers', prefix: 'batch:' }
		];
		const onChange = renderSwitch({ options });
		const disabled = page.getByRole('radio', { name: 'spooky_sketches' });
		await expect.element(disabled).toBeDisabled();
		await disabled.click({ force: true });
		expect(onChange).not.toHaveBeenCalled();
		radio('All').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onChange.mock.calls).toEqual([['stickers']]);
	});

	it('asks nothing while the whole switch is disabled', async () => {
		const onChange = renderSwitch({ disabled: true });
		const group = page.getByRole('radiogroup');
		await expect.element(group).toHaveAttribute('aria-disabled', 'true');
		await expect.element(page.getByRole('radio', { name: 'stickers' })).toBeDisabled();
		await page.getByRole('radio', { name: 'stickers' }).click({ force: true });
		expect(onChange).not.toHaveBeenCalled();
	});

	it('renders a hidden input only when given a name', () => {
		const plain = render(AcceptingPillSwitch<Batch>, {
			props: {
				label: 'Batch',
				options: batches,
				initial: 'all',
				accepts: true,
				onChange: () => undefined
			}
		});
		expect(plain.container.querySelectorAll('input')).toHaveLength(0);
		const named = render(AcceptingPillSwitch<Batch>, {
			props: {
				label: 'Batch',
				options: batches,
				initial: 'stickers',
				accepts: true,
				name: 'batch',
				onChange: () => undefined
			}
		});
		const inputs = [...named.container.querySelectorAll('input')];
		expect(inputs.map((input) => [input.name, input.value])).toEqual([['batch', 'stickers']]);
	});

	it('puts its test id on the group', async () => {
		renderSwitch({ testid: 'batch-filter' });
		await expect.element(page.getByTestId('batch-filter')).toHaveAttribute('role', 'radiogroup');
	});

	it('shows markup in its words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		const options: readonly PillOption<Batch>[] = [
			{ id: 'all', label: words, prefix: words },
			{ id: 'stickers', label: 'stickers' }
		];
		const screen = render(AcceptingPillSwitch<Batch>, {
			props: { label: words, options, initial: 'all', accepts: true, onChange: () => undefined }
		});
		await expect.element(page.getByRole('radiogroup', { name: words })).toBeInTheDocument();
		expect(radio(words).textContent).toContain(words);
		expect(screen.container.querySelector('.zf-seg__type')?.textContent).toBe(words);
		expect(screen.container.querySelector('img')).toBeNull();
	});

	it('shows markup in a type among mixed types as text', () => {
		const words = '<img src=x onerror=alert(1)>';
		const options: readonly PillOption<Batch>[] = [
			{ id: 'all', label: 'All', prefix: words },
			{ id: 'stickers', label: 'stickers', prefix: 'phase:' }
		];
		const screen = render(AcceptingPillSwitch<Batch>, {
			props: { label: 'Batch', options, initial: 'all', accepts: true, onChange: () => undefined }
		});
		expect(screen.container.querySelector('.zf-seg__prefix')?.textContent).toBe(words);
		expect(screen.container.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onChange = renderSwitch();
		await page.getByRole('radio', { name: 'stickers' }).click();
		expect(onChange).toHaveBeenCalledWith('stickers');
	});

	it('squishes each option and plays the toggle cue on a change, and nothing else', async () => {
		const calls: SeamCall[] = [];
		renderSwitch({}, (call) => calls.push(call));
		const squishes = page
			.getByRole('radio')
			.elements()
			.map((element) => ({ kind: 'move', move: 'squish', args: [], element }));
		expect(calls).toEqual(squishes);
		radio('All').focus();
		await userEvent.keyboard('{ArrowRight}');
		await page.getByRole('radio', { name: 'stickers' }).click();
		expect(calls.slice(squishes.length)).toEqual([
			{ kind: 'sound', cue: 'toggle' },
			{ kind: 'sound', cue: 'toggle' }
		]);
	});

	it('wears one piece of picks, the chosen one on its skin’s selected colours', () => {
		renderSwitch({ initial: 'stickers' });
		const group = page.getByRole('radiogroup').element();
		expect([...group.classList].sort()).toEqual(['piece', 'zf-seg']);
		expect(radio('All').className.split(' ').sort()).toEqual(['pick', 'zf-seg__opt']);
	});

	it.each([
		['light', 'rgb(134, 88, 18)', 'rgb(245, 225, 179)'],
		['dark', 'rgb(231, 182, 86)', 'rgb(61, 53, 40)']
	])('shows the chosen option in the %s skin’s accent', (skin, text, tint) => {
		const copy = document.createElement('div');
		copy.setAttribute('data-skin', skin);
		copy.setAttribute('data-copy', '');
		document.body.append(copy);
		renderSwitch({ initial: 'stickers' }, undefined, copy);
		const chosen = getComputedStyle(radio('stickers'));
		expect([chosen.color, chosen.backgroundColor, chosen.fontWeight]).toEqual([text, tint, '700']);
		const resting = getComputedStyle(radio('All'));
		expect(resting.color).not.toBe(text);
		expect(resting.fontWeight).toBe('400');
		expect(resting.backgroundColor).toBe('rgba(0, 0, 0, 0)');
	});
});
