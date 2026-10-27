import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync, tick, type ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import ScrollBoxHost from '../../checks/testing/ScrollBoxHost.svelte';
import Select from './Select.svelte';
import type { SelectOption } from './types';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

type Visibility = 'public' | 'followers' | 'private';

const VISIBILITY: readonly SelectOption<Visibility>[] = [
	{ value: 'public', label: 'Everyone' },
	{ value: 'followers', label: 'Followers only' },
	{ value: 'private', label: 'Only me' }
];

type Stage = 'sketch' | 'lineart' | 'colour' | 'final';

const STAGES: readonly SelectOption<Stage>[] = [
	{ value: 'sketch', label: 'Sketch' },
	{ value: 'lineart', label: 'Lineart' },
	{ value: 'colour', label: 'Colour' },
	{ value: 'final', label: 'Final' }
];

/** The select's trigger, the one combobox on the page. */
function theTrigger(): HTMLButtonElement {
	const [found] = page.getByRole('combobox').elements();
	if (!(found instanceof HTMLButtonElement)) throw new Error('no select trigger on the page');
	return found;
}

/** The open list, wherever it portalled, or undefined while it is closed. */
function openList(): HTMLElement | undefined {
	const [found] = page.getByRole('listbox').elements();
	return found instanceof HTMLElement ? found : undefined;
}

/** The option named `name` in the open list. */
function option(name: string): HTMLElement {
	const [found] = page.getByRole('option', { name }).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`no option ${name}`);
	return found;
}

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** Every hidden input the select renders, inside `root`. */
function hiddenInputs(root: ParentNode = document): readonly HTMLInputElement[] {
	return [...root.querySelectorAll('input')];
}

/** Waits `ms` milliseconds. */
function pause(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

/** A primary mouse button's pointer event of `type`, dispatched on `target` as a browser does. */
function pointer(type: 'pointerdown' | 'pointerup' | 'pointercancel', target: HTMLElement): void {
	const init: PointerEventInit = {
		bubbles: true,
		cancelable: true,
		composed: true,
		button: 0,
		buttons: type === 'pointerdown' ? 1 : 0,
		pointerType: 'mouse',
		isPrimary: true
	};
	target.dispatchEvent(new PointerEvent(type, init));
}

/** Opens the list by pressing the trigger, as a mouse does, and waits for it. */
async function pressOpen(): Promise<HTMLElement> {
	pointer('pointerdown', theTrigger());
	await expect.element(page.getByRole('listbox')).toBeInTheDocument();
	const list = openList();
	if (list === undefined) throw new Error('the list did not open');
	return list;
}

/** Opens the list from the keyboard and waits for it. */
async function keyOpen(): Promise<HTMLElement> {
	theTrigger().focus();
	await userEvent.keyboard('{Enter}');
	await expect.element(page.getByRole('listbox')).toBeInTheDocument();
	const list = openList();
	if (list === undefined) throw new Error('the list did not open');
	return list;
}

/** Waits for the list to close. */
async function closed(): Promise<void> {
	await vi.waitFor(() => {
		expect(openList()).toBeUndefined();
	});
}

/** A form in the page, holding whatever renders into it, whose submits are counted and stopped. */
function aForm(): { readonly form: HTMLFormElement; readonly onSubmit: ReturnType<typeof vi.fn> } {
	const form = document.createElement('form');
	form.setAttribute('data-form', '');
	const onSubmit = vi.fn((event: Event) => {
		event.preventDefault();
	});
	form.addEventListener('submit', onSubmit);
	document.body.append(form);
	return { form, onSubmit };
}

/** The colour `token` gives inside `scope`, read off a probe. */
function tokenColour(token: string, scope: Element = document.body): string {
	const probe = document.createElement('span');
	probe.style.color = `var(${token})`;
	scope.append(probe);
	const colour = getComputedStyle(probe).color;
	probe.remove();
	return colour;
}

/** A copy in the page wearing `skin`, removed after each test. */
function aCopy(skin: 'dark' | 'light'): HTMLElement {
	const copy = document.createElement('div');
	copy.setAttribute('data-skin', skin);
	copy.setAttribute('data-copy', '');
	document.body.append(copy);
	return copy;
}

/** An outline as computed: its style, width, colour and offset. */
interface Ring {
	readonly style: string;
	readonly width: string;
	readonly colour: string;
	readonly offset: string;
}

/** The outline `element` shows, as computed. */
function ringOf(element: Element): Ring {
	const style = getComputedStyle(element);
	return {
		style: style.outlineStyle,
		width: style.outlineWidth,
		colour: style.outlineColor,
		offset: style.outlineOffset
	};
}

/** The one focus ring a highlighted option shows, in `colour`. */
function theRing(colour: string): Ring {
	return { style: 'solid', width: '2px', colour, offset: '-2px' };
}

/** How far `element` is raised off its place by its `translate`, in pixels. */
function riseOf(element: Element): number {
	const translate = getComputedStyle(element).translate;
	if (translate === 'none') return 0;
	const [, y = '0px'] = translate.split(' ');
	return 0 - Number.parseFloat(y);
}

/** The viewport's box, in the page's coordinates. */
function theViewport(): DOMRect {
	return new DOMRect(0, 0, window.innerWidth, window.innerHeight);
}

/** Whether `inner` lies wholly inside `outer`, give or take a pixel. */
function liesInside(inner: DOMRect, outer: DOMRect): boolean {
	return (
		inner.top >= outer.top - 1 &&
		inner.left >= outer.left - 1 &&
		inner.bottom <= outer.bottom + 1 &&
		inner.right <= outer.right + 1
	);
}

afterEach(() => {
	document.documentElement.removeAttribute('data-skin');
	for (const left of document.querySelectorAll('[data-copy], [data-form]')) left.remove();
});

describe('Select', () => {
	it('is a combobox named by its label, described by its hint and its error', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined,
			hint: 'You can change this later',
			error: 'Pick who sees it'
		});
		const trigger = page.getByRole('combobox', { name: 'Who can see this' });
		await expect
			.element(trigger)
			.toHaveAccessibleDescription('You can change this later Pick who sees it');
		await expect.element(trigger).toHaveAttribute('aria-invalid', 'true');
	});

	it('is neither described nor invalid without a hint or an error', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		await expect.element(page.getByRole('combobox')).toBeInTheDocument();
		expect(theTrigger().hasAttribute('aria-describedby')).toBe(false);
		expect(theTrigger().hasAttribute('aria-invalid')).toBe(false);
	});

	it('is a button combobox that points at its list only while the list is open', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		const trigger = theTrigger();
		expect(trigger.getAttribute('role')).toBe('combobox');
		expect(trigger.getAttribute('type')).toBe('button');
		expect(trigger.getAttribute('aria-expanded')).toBe('false');
		expect(trigger.hasAttribute('aria-controls')).toBe(false);
		await page.getByRole('combobox').click();
		await expect.element(page.getByRole('listbox', { name: 'Who can see this' })).toBeVisible();
		expect(trigger.getAttribute('aria-expanded')).toBe('true');
		const list = openList();
		expect(list?.id).not.toBe('');
		expect(trigger.getAttribute('aria-controls')).toBe(list?.id);
		await userEvent.keyboard('{Escape}');
		await closed();
		expect(trigger.hasAttribute('aria-controls')).toBe(false);
	});

	it('shows the label the value names, and lists every option in order, the chosen one checked', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'followers',
			onChange: () => undefined
		});
		await expect.element(page.getByRole('combobox')).toHaveTextContent('Followers only');
		await page.getByRole('combobox').click();
		await expect.element(page.getByRole('listbox')).toBeVisible();
		const names = page
			.getByRole('option')
			.elements()
			.map((row) => row.textContent.trim());
		expect(names).toEqual(['Everyone', 'Followers only', 'Only me']);
		expect(option('Followers only').getAttribute('aria-selected')).toBe('true');
		expect(option('Followers only').querySelector('svg')).not.toBeNull();
		expect(option('Everyone').hasAttribute('aria-selected')).toBe(false);
		expect(option('Everyone').querySelector('svg')).toBeNull();
	});

	it('shows its placeholder in muted ink while nothing is chosen', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: undefined,
			onChange: () => undefined,
			placeholder: 'Choose who sees it'
		});
		await expect.element(page.getByRole('combobox')).toHaveTextContent('Choose who sees it');
		const shown = theTrigger().querySelector('.zf-select__value');
		expect(shown === null ? '' : getComputedStyle(shown).color).toBe(
			tokenColour('--color-text-muted')
		);
	});

	it('shows no words while nothing is chosen and there is no placeholder', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: undefined,
			onChange: () => undefined
		});
		await expect.element(page.getByRole('combobox')).toBeInTheDocument();
		expect(theTrigger().textContent.trim()).toBe('');
	});

	it('hands the parent the option value, typed, when one is clicked, and closes', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await page.getByRole('combobox').click();
		await page.getByRole('option', { name: 'Only me' }).click();
		expect(onChange.mock.calls).toEqual([['private']]);
		await closed();
	});

	it('chooses with the keyboard, keeping focus on the trigger', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await keyOpen();
		expect(document.activeElement).toBe(theTrigger());
		await vi.waitFor(() => {
			expect(option('Everyone').hasAttribute('data-highlighted')).toBe(true);
		});
		await userEvent.keyboard('{ArrowDown}');
		expect(option('Followers only').hasAttribute('data-highlighted')).toBe(true);
		await userEvent.keyboard('{Enter}');
		expect(onChange.mock.calls).toEqual([['followers']]);
		await closed();
		expect(document.activeElement).toBe(theTrigger());
	});

	it.each([['dark'], ['light']] as const)(
		'rings the highlighted option in the %s skin, the chosen one too, and lifts one not yet chosen',
		async (skin) => {
			const copy = aCopy(skin);
			render(Select<Visibility>, {
				target: copy,
				props: {
					label: 'Who can see this',
					options: VISIBILITY,
					value: 'public',
					onChange: () => undefined
				}
			});
			const focus = tokenColour('--color-focus', copy);
			await keyOpen();
			await vi.waitFor(() => {
				expect(option('Everyone').hasAttribute('data-highlighted')).toBe(true);
			});
			expect(option('Everyone').getAttribute('aria-selected')).toBe('true');
			expect(ringOf(option('Everyone'))).toEqual(theRing(focus));
			expect(ringOf(option('Followers only')).style).toBe('none');
			expect(ringOf(option('Only me')).style).toBe('none');
			const resting = getComputedStyle(option('Only me')).boxShadow;
			await userEvent.keyboard('{ArrowDown}');
			expect(option('Followers only').hasAttribute('data-highlighted')).toBe(true);
			expect(ringOf(option('Followers only'))).toEqual(theRing(focus));
			expect(ringOf(option('Everyone')).style).toBe('none');
			expect(ringOf(option('Only me')).style).toBe('none');
			await vi.waitFor(() => {
				const lit = option('Followers only');
				expect(getComputedStyle(lit).boxShadow).not.toBe(resting);
				expect(riseOf(lit)).toBeGreaterThan(0);
			});
		}
	);

	it('rings each skin in its own focus colour', () => {
		expect(tokenColour('--color-focus', aCopy('dark'))).not.toBe(
			tokenColour('--color-focus', aCopy('light'))
		);
	});

	it('keeps the ring on the highlighted option under Still, where the lift is gone', async () => {
		const copy = aCopy('dark');
		copy.setAttribute('data-motion', 'still');
		render(Select<Visibility>, {
			target: copy,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			}
		});
		const focus = tokenColour('--color-focus', copy);
		const list = await keyOpen();
		expect(list.closest('[data-motion="still"]')).toBe(copy);
		await vi.waitFor(() => {
			expect(ringOf(option('Everyone'))).toEqual(theRing(focus));
		});
		await userEvent.keyboard('{ArrowDown}');
		expect(option('Followers only').hasAttribute('data-highlighted')).toBe(true);
		expect(ringOf(option('Followers only'))).toEqual(theRing(focus));
		expect(riseOf(option('Followers only'))).toBe(0);
		expect(ringOf(option('Everyone')).style).toBe('none');
	});

	it('moves the ring with the pointer', async () => {
		const copy = aCopy('dark');
		render(Select<Visibility>, {
			target: copy,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			}
		});
		const focus = tokenColour('--color-focus', copy);
		await keyOpen();
		await userEvent.hover(option('Only me'));
		await vi.waitFor(() => {
			expect(option('Only me').hasAttribute('data-highlighted')).toBe(true);
		});
		expect(ringOf(option('Only me'))).toEqual(theRing(focus));
		expect(ringOf(option('Everyone')).style).toBe('none');
		await userEvent.unhover(option('Only me'));
	});

	it('closes on Escape, choosing nothing and keeping focus', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await keyOpen();
		await userEvent.keyboard('{ArrowDown}');
		await userEvent.keyboard('{Escape}');
		await closed();
		expect(onChange).not.toHaveBeenCalled();
		expect(document.activeElement).toBe(theTrigger());
	});

	it('chooses by typing a letter while closed', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		theTrigger().focus();
		await userEvent.keyboard('f');
		expect(onChange.mock.calls).toEqual([['followers']]);
		expect(openList()).toBeUndefined();
	});

	it('focuses the trigger when its label is clicked, without opening', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		await page.getByText('Who can see this').click();
		expect(document.activeElement).toBe(theTrigger());
		await pause(100);
		expect(openList()).toBeUndefined();
	});

	it('shows the old option when the parent refuses', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await page.getByRole('combobox').click();
		await page.getByRole('option', { name: 'Only me' }).click();
		flushSync();
		await tick();
		expect(onChange.mock.calls).toEqual([['private']]);
		await expect.element(page.getByRole('combobox')).toHaveTextContent('Everyone');
		await closed();
		await page.getByRole('combobox').click();
		await expect.element(page.getByRole('listbox')).toBeVisible();
		expect(option('Everyone').getAttribute('aria-selected')).toBe('true');
		expect(option('Only me').hasAttribute('aria-selected')).toBe(false);
	});

	it('shows the placeholder again when the parent refuses the first choice', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: undefined,
			onChange: () => undefined,
			placeholder: 'Choose who sees it'
		});
		await page.getByRole('combobox').click();
		await page.getByRole('option', { name: 'Followers only' }).click();
		flushSync();
		await tick();
		await expect.element(page.getByRole('combobox')).toHaveTextContent('Choose who sees it');
	});

	it('follows the prop when the parent accepts', async () => {
		const screen = render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		await screen.rerender({ value: 'private' });
		await expect.element(page.getByRole('combobox')).toHaveTextContent('Only me');
	});

	it('shows the right option when the options change under it', async () => {
		const screen = render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'private',
			onChange: () => undefined
		});
		await screen.rerender({
			options: [{ value: 'private', label: 'Just me' }, ...VISIBILITY.slice(0, 2)]
		});
		await expect.element(page.getByRole('combobox')).toHaveTextContent('Just me');
	});

	it('asks nothing when the option already shown is chosen again', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await page.getByRole('combobox').click();
		await page.getByRole('option', { name: 'Everyone' }).click();
		await closed();
		expect(onChange).not.toHaveBeenCalled();
	});

	it('hands back the typed value of the option, not the text it was matched by', async () => {
		const options: readonly SelectOption<'1' | '2'>[] = [
			{ value: '1', label: 'Sketch' },
			{ value: '2', label: 'Final' }
		];
		const onChange = vi.fn<(value: '1' | '2') => void>();
		render(Select<'1' | '2'>, { label: 'Stage', options, value: '1', onChange });
		await page.getByRole('combobox').click();
		await page.getByRole('option', { name: 'Final' }).click();
		expect(onChange.mock.calls).toEqual([['2']]);
		expect(options.map((each) => each.value)).toContain(onChange.mock.calls[0]?.[0]);
	});

	it('asks nothing while disabled, by pointer or key', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange,
			disabled: true
		});
		await expect.element(page.getByRole('combobox')).toBeDisabled();
		pointer('pointerdown', theTrigger());
		pointer('pointerup', theTrigger());
		theTrigger().focus();
		await userEvent.keyboard('{ArrowDown}');
		await userEvent.keyboard('f');
		await pause(100);
		expect(openList()).toBeUndefined();
		expect(onChange).not.toHaveBeenCalled();
	});

	it('is marked required for screen readers, and never blocks a form in the browser', async () => {
		const { form } = aForm();
		render(Select<Visibility>, {
			target: form,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: undefined,
				onChange: () => undefined,
				required: true,
				name: 'visibility'
			}
		});
		await expect.element(page.getByRole('combobox')).toHaveAttribute('aria-required', 'true');
		expect(form.querySelectorAll('[required]')).toHaveLength(0);
		expect(form.checkValidity()).toBe(true);
	});

	it('is not marked required unless asked', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		await expect.element(page.getByRole('combobox')).toBeInTheDocument();
		expect(theTrigger().hasAttribute('aria-required')).toBe(false);
	});

	it('renders a hidden input only with a name and a chosen value', async () => {
		const { form } = aForm();
		const nameless = render(Select<Visibility>, {
			target: form,
			props: {
				label: 'No name',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			}
		});
		expect(hiddenInputs(form)).toHaveLength(0);
		nameless.unmount();
		const unchosen = render(Select<Visibility>, {
			target: form,
			props: {
				label: 'Nothing chosen',
				options: VISIBILITY,
				value: undefined,
				onChange: () => undefined,
				placeholder: 'Choose who sees it',
				name: 'visibility'
			}
		});
		expect(hiddenInputs(form)).toHaveLength(0);
		expect(new FormData(form).has('visibility')).toBe(false);
		await unchosen.rerender({ value: 'followers' });
		expect(hiddenInputs(form)).toHaveLength(1);
		expect(new FormData(form).get('visibility')).toBe('followers');
	});

	it.each([
		['a string that is no option', 'evil'],
		['the empty string', '']
	])('ignores %s written into its hidden input, and posts the value it shows', async (_, raw) => {
		const { form } = aForm();
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			target: form,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'followers',
				onChange,
				name: 'visibility'
			}
		});
		const [input] = hiddenInputs(form);
		if (input === undefined) throw new Error('no hidden input');
		input.value = raw;
		input.dispatchEvent(new Event('input', { bubbles: true }));
		await tick();
		await tick();
		expect(onChange).not.toHaveBeenCalled();
		expect(new FormData(form).get('visibility')).toBe('followers');
		await expect.element(page.getByRole('combobox')).toHaveTextContent('Followers only');
	});

	it.each([
		['a click', async () => page.getByRole('combobox').click()],
		[
			'Enter',
			async () => {
				theTrigger().focus();
				await userEvent.keyboard('{Enter}');
			}
		],
		[
			'Space',
			async () => {
				theTrigger().focus();
				await userEvent.keyboard(' ');
			}
		]
	])('never submits its form on %s', async (_, press) => {
		const { form, onSubmit } = aForm();
		render(Select<Visibility>, {
			target: form,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined,
				name: 'visibility'
			}
		});
		await press();
		await expect.element(page.getByRole('listbox')).toBeInTheDocument();
		await pause(100);
		expect(onSubmit).not.toHaveBeenCalled();
		expect(theTrigger().type).toBe('button');
	});

	it('chooses on a press and a release on the same option', async () => {
		const onChange = vi.fn<(value: Stage) => void>();
		render(Select<Stage>, { label: 'Stage', options: STAGES, value: 'sketch', onChange });
		await pressOpen();
		pointer('pointerdown', option('Lineart'));
		pointer('pointerup', option('Lineart'));
		expect(onChange.mock.calls).toEqual([['lineart']]);
	});

	it('chooses nothing on a release over an option the press did not start on', async () => {
		const onChange = vi.fn<(value: Stage) => void>();
		render(Select<Stage>, { label: 'Stage', options: STAGES, value: 'sketch', onChange });
		const list = await pressOpen();
		pointer('pointerup', option('Lineart'));
		await pause(100);
		expect(onChange).not.toHaveBeenCalled();
		expect(openList()).toBe(list);
		pointer('pointerdown', option('Colour'));
		pointer('pointerup', option('Final'));
		await pause(100);
		expect(onChange).not.toHaveBeenCalled();
		expect(openList()).toBe(list);
	});

	it('forgets a refused choice: reopening and releasing on it chooses nothing', async () => {
		const onChange = vi.fn<(value: Stage) => void>();
		render(Select<Stage>, { label: 'Stage', options: STAGES, value: 'sketch', onChange });
		await pressOpen();
		pointer('pointerdown', option('Lineart'));
		pointer('pointerup', option('Lineart'));
		expect(onChange.mock.calls).toEqual([['lineart']]);
		await closed();
		await pressOpen();
		pointer('pointerup', option('Lineart'));
		await pause(100);
		expect(onChange.mock.calls).toEqual([['lineart']]);
	});

	it('forgets a press after every release, matched or not', async () => {
		const onChange = vi.fn<(value: Stage) => void>();
		render(Select<Stage>, { label: 'Stage', options: STAGES, value: 'sketch', onChange });
		await pressOpen();
		pointer('pointerdown', option('Colour'));
		pointer('pointerup', option('Final'));
		pointer('pointerup', option('Colour'));
		await pause(100);
		expect(onChange).not.toHaveBeenCalled();
	});

	it('forgets a press across a close and an open', async () => {
		const onChange = vi.fn<(value: Stage) => void>();
		render(Select<Stage>, { label: 'Stage', options: STAGES, value: 'sketch', onChange });
		await keyOpen();
		pointer('pointerdown', option('Colour'));
		await userEvent.keyboard('{Escape}');
		await closed();
		await userEvent.keyboard('{Enter}');
		await expect.element(page.getByRole('listbox')).toBeInTheDocument();
		pointer('pointerup', option('Colour'));
		await pause(100);
		expect(onChange).not.toHaveBeenCalled();
	});

	it('forgets a cancelled press', async () => {
		const onChange = vi.fn<(value: Stage) => void>();
		render(Select<Stage>, { label: 'Stage', options: STAGES, value: 'sketch', onChange });
		await pressOpen();
		pointer('pointerdown', option('Colour'));
		pointer('pointercancel', option('Colour'));
		pointer('pointerup', option('Colour'));
		await pause(100);
		expect(onChange).not.toHaveBeenCalled();
	});

	it('forgets a press when the next one starts off every option', async () => {
		const onChange = vi.fn<(value: Stage) => void>();
		render(Select<Stage>, { label: 'Stage', options: STAGES, value: 'sketch', onChange });
		const list = await pressOpen();
		pointer('pointerdown', option('Colour'));
		pointer('pointerup', list);
		pointer('pointerdown', list);
		pointer('pointerup', option('Colour'));
		await pause(100);
		expect(onChange).not.toHaveBeenCalled();
	});

	it('scrolls nothing outside its list as it opens, by pointer or by key', async () => {
		render(ScrollBoxHost, {});
		const box = byTestId('scroll-box');
		const scope = byTestId('scroll-box-scope');
		render(Select<Stage>, {
			target: scope,
			props: { label: 'Stage', options: STAGES, value: 'final', onChange: () => undefined }
		});
		await page.getByRole('combobox').click();
		const list = openList();
		await expect.element(page.getByRole('listbox')).toBeVisible();
		expect(list?.closest('[data-testid="scroll-box"]')).toBe(box);
		expect(box.scrollHeight).toBeGreaterThan(box.clientHeight);
		await pause(150);
		expect(box.scrollTop).toBe(0);
		await userEvent.keyboard('{Escape}');
		await closed();
		await userEvent.keyboard('{ArrowDown}');
		await expect.element(page.getByRole('listbox')).toBeVisible();
		await userEvent.keyboard('{Home}');
		await userEvent.keyboard('{End}');
		await pause(150);
		expect(box.scrollTop).toBe(0);
	});

	it('scrolls a long list inside itself to the highlighted option', async () => {
		const many: readonly SelectOption<string>[] = Array.from({ length: 20 }, (_, index) => ({
			value: `stage-${String(index)}`,
			label: `Stage ${String(index)}`
		}));
		render(Select<string>, {
			label: 'Stage',
			options: many,
			value: 'stage-0',
			onChange: () => undefined
		});
		const list = await keyOpen();
		expect(list.scrollHeight).toBeGreaterThan(list.clientHeight);
		expect(list.scrollTop).toBe(0);
		await userEvent.keyboard('{End}');
		await vi.waitFor(() => {
			const last = option('Stage 19').getBoundingClientRect();
			const box = list.getBoundingClientRect();
			expect(last.bottom).toBeLessThanOrEqual(box.bottom + 1);
		});
		expect(list.scrollTop).toBeGreaterThan(0);
		expect(document.scrollingElement?.scrollTop ?? 0).toBe(0);
	});

	it('caps its list by the room on screen, scrolling inside, with the last option shown whole', async () => {
		const many: readonly SelectOption<string>[] = Array.from({ length: 12 }, (_, index) => ({
			value: `stage-${String(index + 1)}`,
			label: `Stage ${String(index + 1)}`
		}));
		const roomy = { width: window.innerWidth, height: window.innerHeight };
		await page.viewport(320, 256);
		try {
			render(Select<string>, {
				label: 'Stage',
				options: many,
				value: 'stage-1',
				onChange: () => undefined
			});
			await userEvent.unhover(theTrigger());
			await vi.waitFor(() => {
				expect(riseOf(theTrigger())).toBe(0);
			});
			const list = await keyOpen();
			const wrapper = list.parentElement;
			if (wrapper === null) throw new Error('the list has no wrapper');
			await vi.waitFor(() => {
				expect(liesInside(list.getBoundingClientRect(), theViewport())).toBe(true);
				expect(liesInside(wrapper.getBoundingClientRect(), theViewport())).toBe(true);
			});
			expect(list.scrollHeight).toBeGreaterThan(list.clientHeight);
			for (let step = 1; step < many.length; step += 1) {
				await userEvent.keyboard('{ArrowDown}');
			}
			expect(option('Stage 12').hasAttribute('data-highlighted')).toBe(true);
			await vi.waitFor(() => {
				const box = list.getBoundingClientRect();
				const last = option('Stage 12').getBoundingClientRect();
				expect(last.top).toBeGreaterThanOrEqual(box.top - 1);
				expect(last.bottom).toBeLessThanOrEqual(box.bottom + 1);
			});
			expect(liesInside(list.getBoundingClientRect(), theViewport())).toBe(true);
		} finally {
			await page.viewport(roomy.width, roomy.height);
		}
	});

	it.each([
		['light', 'dark'],
		['dark', 'light']
	])('opens inside a %s copy on a %s page, on that copy’s note colour', async (skin, pageSkin) => {
		document.documentElement.setAttribute('data-skin', pageSkin);
		const copy = document.createElement('div');
		copy.setAttribute('data-skin', skin);
		copy.setAttribute('data-copy', '');
		document.body.append(copy);
		render(Select<Visibility>, {
			target: copy,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			}
		});
		await page.getByRole('combobox').click();
		await expect.element(page.getByRole('listbox')).toBeVisible();
		const list = openList();
		expect(list?.closest('[data-skin]')).toBe(copy);
		expect(list === undefined ? '' : getComputedStyle(list).backgroundColor).toBe(
			tokenColour('--color-note', copy)
		);
		expect(tokenColour('--color-note', copy)).not.toBe(tokenColour('--color-note'));
	});

	it('puts its list down as a note in place, and cues the open and the close', async () => {
		const calls: SeamCall[] = [];
		render(
			Select<Visibility>,
			{
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			},
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(calls).toEqual([]);
		await page.getByRole('combobox').click();
		await expect.element(page.getByRole('listbox')).toBeVisible();
		const list = openList();
		const wrapper = list?.parentElement;
		expect(calls).toEqual([
			{ kind: 'sound', cue: 'open' },
			{ kind: 'move', move: 'note', args: ['in-place'], element: list }
		]);
		expect(wrapper?.classList).toHaveLength(0);
		expect(wrapper?.hasAttribute('role')).toBe(false);
		await page.getByRole('option', { name: 'Only me' }).click();
		await closed();
		expect(calls.slice(2)).toEqual([{ kind: 'sound', cue: 'close' }]);
	});

	it('lifts its slip only while it can be changed', () => {
		const enabled = render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		expect(theTrigger().classList.contains('lift')).toBe(true);
		enabled.unmount();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined,
			disabled: true
		});
		expect(theTrigger().classList.contains('lift')).toBe(false);
	});

	it('lifts its slip on point while it can be changed, and never squishes', async () => {
		const calls: SeamCall[] = [];
		render(
			Select<Visibility>,
			{
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			},
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(riseOf(theTrigger())).toBe(0);
		await userEvent.hover(theTrigger());
		await vi.waitFor(() => {
			expect(riseOf(theTrigger())).toBeGreaterThan(0);
		});
		await pressOpen();
		pointer('pointerup', theTrigger());
		await userEvent.keyboard('{Escape}');
		await closed();
		await page.getByRole('combobox').click();
		await page.getByRole('option', { name: 'Only me' }).click();
		await closed();
		await userEvent.unhover(theTrigger());
		expect(calls.filter((call) => call.kind === 'sound')).toHaveLength(4);
		expect(calls.filter((call) => call.kind === 'move' && call.move === 'squish')).toEqual([]);
	});

	it('stays down on point while disabled', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined,
			disabled: true
		});
		await userEvent.hover(theTrigger());
		expect(theTrigger().matches(':hover')).toBe(true);
		await pause(300);
		expect(riseOf(theTrigger())).toBe(0);
		await userEvent.unhover(theTrigger());
	});

	it('puts its test id on the trigger', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined,
			testid: 'visibility-select'
		});
		await expect.element(page.getByTestId('visibility-select')).toHaveRole('combobox');
	});

	it('shows markup in its words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		render(Select<'a' | 'b'>, {
			label: words,
			options: [
				{ value: 'a', label: words },
				{ value: 'b', label: 'Other' }
			],
			value: undefined,
			onChange: () => undefined,
			hint: words,
			error: words,
			placeholder: words
		});
		await expect.element(page.getByRole('combobox', { name: words })).toHaveTextContent(words);
		await page.getByRole('combobox').click();
		await expect.element(page.getByRole('option', { name: words })).toBeInTheDocument();
		expect(document.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await page.getByRole('combobox').click();
		await page.getByRole('option', { name: 'Followers only' }).click();
		expect(onChange).toHaveBeenCalledWith('followers');
	});

	it('wears the slip with a chevron and the one focus ring, its list the menu’s note and pick rows', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		const trigger = theTrigger();
		expect(trigger.classList.contains('zf-select')).toBe(true);
		expect(trigger.classList.contains('slip')).toBe(true);
		expect(trigger.querySelector('.zf-select__chevron svg')).not.toBeNull();
		trigger.focus();
		const ring = getComputedStyle(trigger);
		expect(ring.outlineStyle).toBe('solid');
		expect(ring.outlineWidth).toBe('2px');
		await page.getByRole('combobox').click();
		await expect.element(page.getByRole('listbox')).toHaveClass('zf-menu');
		expect(option('Everyone').classList.contains('zf-menu__item')).toBe(true);
		expect(option('Everyone').classList.contains('pick')).toBe(true);
	});

	it.each([['dark'], ['light']] as const)('takes its colours from the %s skin', (skin) => {
		const scope = document.createElement('div');
		scope.setAttribute('data-skin', skin);
		scope.setAttribute('data-copy', '');
		document.body.append(scope);
		render(Select<Visibility>, {
			target: scope,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			}
		});
		expect(getComputedStyle(theTrigger()).backgroundColor).toBe(
			tokenColour('--color-object', scope)
		);
	});
});
