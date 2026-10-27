import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync, tick, type ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
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

/** The one select on the page. */
function theSelect(): HTMLSelectElement {
	const found = document.querySelector('select');
	if (!(found instanceof HTMLSelectElement)) throw new Error('no select on the page');
	return found;
}

/** Sets the select to a raw string, as the DOM could hand it back, and fires its change. */
function chooseRaw(raw: string): void {
	const select = theSelect();
	select.value = raw;
	select.dispatchEvent(new Event('change', { bubbles: true }));
}

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
		const select = page.getByRole('combobox', { name: 'Who can see this' });
		await expect
			.element(select)
			.toHaveAccessibleDescription('You can change this later Pick who sees it');
		await expect.element(select).toHaveAttribute('aria-invalid', 'true');
	});

	it('is neither described nor invalid without a hint or an error', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		await expect.element(page.getByRole('combobox')).toBeInTheDocument();
		expect(theSelect().hasAttribute('aria-describedby')).toBe(false);
		expect(theSelect().hasAttribute('aria-invalid')).toBe(false);
	});

	it('lists its options by their labels and shows the one the value names', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'followers',
			onChange: () => undefined
		});
		const names = [...theSelect().options].map((option) => option.textContent);
		expect(names).toEqual(['Everyone', 'Followers only', 'Only me']);
		await expect.element(page.getByRole('combobox')).toHaveDisplayValue('Followers only');
	});

	it('shows its placeholder, disabled, while nothing is chosen', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: undefined,
			onChange: () => undefined,
			placeholder: 'Choose who sees it'
		});
		const [first] = [...theSelect().options];
		expect(first?.disabled).toBe(true);
		await expect.element(page.getByRole('combobox')).toHaveDisplayValue('Choose who sees it');
	});

	it('shows no option while nothing is chosen and there is no placeholder', () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: undefined,
			onChange: () => undefined
		});
		expect(theSelect().selectedIndex).toBe(-1);
	});

	it('hands the parent the option value, typed, when one is chosen', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await page.getByRole('combobox').selectOptions('private');
		expect(onChange.mock.calls).toEqual([['private']]);
	});

	it('chooses with the keyboard', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		theSelect().focus();
		await userEvent.keyboard('{ArrowDown}');
		expect(onChange.mock.calls).toEqual([['followers']]);
	});

	it('focuses the select when its label is clicked', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		await page.getByText('Who can see this').click();
		expect(document.activeElement).toBe(theSelect());
	});

	it('shows the old option when the parent refuses', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await page.getByRole('combobox').selectOptions('private');
		flushSync();
		await tick();
		expect(onChange).toHaveBeenCalledWith('private');
		expect(theSelect().value).toBe('public');
		await expect.element(page.getByRole('combobox')).toHaveDisplayValue('Everyone');
	});

	it('shows the placeholder again when the parent refuses the first choice', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: undefined,
			onChange: () => undefined,
			placeholder: 'Choose who sees it'
		});
		await page.getByRole('combobox').selectOptions('followers');
		flushSync();
		await tick();
		await expect.element(page.getByRole('combobox')).toHaveDisplayValue('Choose who sees it');
	});

	it('follows the prop when the parent accepts', async () => {
		const screen = render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		await screen.rerender({ value: 'private' });
		expect(theSelect().value).toBe('private');
	});

	it('shows the right option when the options change under it', async () => {
		const screen = render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'private',
			onChange: () => undefined
		});
		await screen.rerender({
			options: [{ value: 'private', label: 'Only me' }, ...VISIBILITY.slice(0, 2)]
		});
		expect(theSelect().value).toBe('private');
		expect(theSelect().selectedIndex).toBe(0);
	});

	it('ignores a value that is no option', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'followers',
			onChange
		});
		chooseRaw('everyone-and-their-dog');
		await tick();
		expect(onChange).not.toHaveBeenCalled();
		expect(theSelect().value).toBe('followers');
	});

	it('ignores the empty value', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'followers',
			onChange,
			placeholder: 'Choose who sees it'
		});
		chooseRaw('');
		await tick();
		expect(onChange).not.toHaveBeenCalled();
		expect(theSelect().value).toBe('followers');
	});

	it('hands back the typed value of the option, not the text it was matched by', async () => {
		const options: readonly SelectOption<'1' | '2'>[] = [
			{ value: '1', label: 'Sketch' },
			{ value: '2', label: 'Final' }
		];
		const onChange = vi.fn<(value: '1' | '2') => void>();
		render(Select<'1' | '2'>, { label: 'Stage', options, value: '1', onChange });
		await page.getByRole('combobox').selectOptions('2');
		expect(onChange.mock.calls).toEqual([['2']]);
		expect(options.map((option) => option.value)).toContain(onChange.mock.calls[0]?.[0]);
	});

	it('asks nothing while disabled', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange,
			disabled: true
		});
		await expect.element(page.getByRole('combobox')).toBeDisabled();
		theSelect().focus();
		await userEvent.keyboard('{ArrowDown}');
		expect(onChange).not.toHaveBeenCalled();
	});

	it('is required when asked to be', async () => {
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined,
			required: true
		});
		await expect.element(page.getByRole('combobox')).toBeRequired();
	});

	it('renders a name only when given one, and never a hidden input', () => {
		const plain = render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		expect(theSelect().hasAttribute('name')).toBe(false);
		expect(plain.container.querySelectorAll('input')).toHaveLength(0);
		const named = render(Select<Visibility>, {
			label: 'Who sees the draft',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined,
			name: 'visibility'
		});
		expect(named.container.querySelector('select')?.getAttribute('name')).toBe('visibility');
		expect(named.container.querySelectorAll('input')).toHaveLength(0);
	});

	it('puts its test id on the select', async () => {
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
		const { container } = render(Select<'a'>, {
			label: words,
			options: [{ value: 'a', label: words }],
			value: 'a',
			onChange: () => undefined,
			hint: words,
			error: words,
			placeholder: words
		});
		await expect.element(page.getByRole('combobox', { name: words })).toBeInTheDocument();
		expect(container.querySelector('img')).toBeNull();
		expect(theSelect().options[0]?.textContent).toBe(words);
	});

	it('works with no environment provided', async () => {
		const onChange = vi.fn<(value: Visibility) => void>();
		render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange
		});
		await page.getByRole('combobox').selectOptions('followers');
		expect(onChange).toHaveBeenCalledWith('followers');
	});

	it('calls nothing in the seam', async () => {
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
		await page.getByRole('combobox').selectOptions('followers');
		expect(calls).toEqual([]);
	});

	it('wears the bundle classes, a slip with a chevron and the one focus ring on the field', () => {
		const { container } = render(Select<Visibility>, {
			label: 'Who can see this',
			options: VISIBILITY,
			value: 'public',
			onChange: () => undefined
		});
		const frame = container.querySelector('.zf-select');
		expect(frame?.classList.contains('slip')).toBe(true);
		expect(container.querySelector('.zf-select__chevron svg')).not.toBeNull();
		theSelect().focus();
		const ring = getComputedStyle(frame ?? document.body);
		expect(ring.outlineStyle).toBe('solid');
		expect(ring.outlineWidth).toBe('2px');
		expect(getComputedStyle(theSelect()).outlineColor).toBe('rgba(0, 0, 0, 0)');
	});

	it.each([['dark'], ['light']] as const)('takes its colours from the %s skin', (skin) => {
		const scope = document.createElement('div');
		scope.setAttribute('data-skin', skin);
		document.body.append(scope);
		const { container } = render(Select<Visibility>, {
			target: scope,
			props: {
				label: 'Who can see this',
				options: VISIBILITY,
				value: 'public',
				onChange: () => undefined
			}
		});
		const probe = document.createElement('span');
		probe.style.color = 'var(--color-object)';
		scope.append(probe);
		const frame = container.querySelector('.zf-select') ?? scope;
		expect(getComputedStyle(frame).backgroundColor).toBe(getComputedStyle(probe).color);
		scope.remove();
	});
});
