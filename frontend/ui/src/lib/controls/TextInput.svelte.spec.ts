import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync, tick, type ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import TextInput from './TextInput.svelte';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The one text field on the page, whether a line or several. */
function theField(): HTMLInputElement | HTMLTextAreaElement {
	const found = document.querySelector('input, textarea');
	if (!(found instanceof HTMLInputElement || found instanceof HTMLTextAreaElement)) {
		throw new Error('no text field on the page');
	}
	return found;
}

/** The text, with a skin around it, in a container the field is rendered into. */
function inSkin(skin: 'dark' | 'light'): HTMLElement {
	const scope = document.createElement('div');
	scope.setAttribute('data-skin', skin);
	document.body.append(scope);
	return scope;
}

/** What a token resolves to in a skin, as the browser writes a colour. */
function tokenColour(scope: HTMLElement, token: string): string {
	const probe = document.createElement('span');
	probe.style.color = `var(${token})`;
	scope.append(probe);
	const colour = getComputedStyle(probe).color;
	probe.remove();
	return colour;
}

/** The pair of states each shape of the field is held to. */
const shapes = [
	['a single line', false],
	['several lines', true]
] as const;

describe('TextInput', () => {
	it.each(shapes)('is a textbox named by its label, as %s', async (_, multiline) => {
		render(TextInput, { label: 'Handle', value: '', onChange: () => undefined, multiline });
		await expect.element(page.getByRole('textbox', { name: 'Handle' })).toBeInTheDocument();
		expect(theField().tagName).toBe(multiline ? 'TEXTAREA' : 'INPUT');
	});

	it.each(shapes)('is described by its hint and its error, as %s', async (_, multiline) => {
		render(TextInput, {
			label: 'Handle',
			value: 'artist',
			onChange: () => undefined,
			multiline,
			hint: 'The name people find you by',
			error: 'That handle is taken. Try another?'
		});
		const field = page.getByRole('textbox', { name: 'Handle' });
		await expect
			.element(field)
			.toHaveAccessibleDescription(
				'The name people find you by That handle is taken. Try another?'
			);
		await expect.element(field).toHaveAttribute('aria-invalid', 'true');
	});

	it('is neither described nor invalid without a hint or an error', async () => {
		render(TextInput, { label: 'Handle', value: '', onChange: () => undefined });
		await expect.element(page.getByRole('textbox')).toBeInTheDocument();
		const field = theField();
		expect(field.hasAttribute('aria-describedby')).toBe(false);
		expect(field.hasAttribute('aria-invalid')).toBe(false);
	});

	it('shows its error with an alert icon beside the words', () => {
		const { container } = render(TextInput, {
			label: 'Handle',
			value: '',
			onChange: () => undefined,
			error: 'That handle is taken. Try another?'
		});
		const error = container.querySelector('.zf-field__error');
		expect(error?.textContent).toBe('That handle is taken. Try another?');
		expect(error?.querySelector('svg')).not.toBeNull();
	});

	it('keeps its label for screen readers only when the label is hidden', async () => {
		const { container } = render(TextInput, {
			label: 'Search commissions',
			labelHidden: true,
			value: '',
			onChange: () => undefined,
			type: 'search'
		});
		await expect.element(page.getByRole('searchbox', { name: 'Search commissions' })).toBeVisible();
		const label = container.querySelector('.zf-field__label');
		expect(label?.getBoundingClientRect().width).toBeLessThanOrEqual(1);
	});

	it('focuses the field when its label is clicked', async () => {
		render(TextInput, { label: 'Handle', value: '', onChange: () => undefined });
		await page.getByText('Handle').click();
		expect(document.activeElement).toBe(theField());
	});

	it.each(shapes)('hands the parent the text, as a plain string, as %s', async (_, multiline) => {
		const onChange = vi.fn<(text: string) => void>();
		render(TextInput, { label: 'Handle', value: '', onChange, multiline });
		await userEvent.type(theField(), 'ab');
		expect(onChange.mock.calls).toEqual([['a'], ['b']]);
	});

	it.each(shapes)('shows the old text when the parent refuses, as %s', async (_, multiline) => {
		const onChange = vi.fn<(text: string) => void>();
		render(TextInput, { label: 'Handle', value: 'artist', onChange, multiline });
		await userEvent.type(theField(), 'x');
		flushSync();
		await tick();
		expect(onChange).toHaveBeenCalledWith('artistx');
		expect(theField().value).toBe('artist');
	});

	it.each(shapes)('follows the prop when the parent accepts, as %s', async (_, multiline) => {
		const screen = render(TextInput, {
			label: 'Handle',
			value: 'artist',
			onChange: () => undefined,
			multiline
		});
		await screen.rerender({ value: 'sketch' });
		expect(theField().value).toBe('sketch');
	});

	it.each(shapes)('forwards the four privacy attributes, as %s', (_, multiline) => {
		render(TextInput, {
			label: 'Recovery phrase',
			value: '',
			onChange: () => undefined,
			multiline,
			autocomplete: 'off',
			autocorrect: 'off',
			spellcheck: false,
			autocapitalize: 'off',
			inputmode: 'text'
		});
		const field = theField();
		expect(field.getAttribute('autocomplete')).toBe('off');
		expect(field.getAttribute('autocorrect')).toBe('off');
		expect(field.getAttribute('spellcheck')).toBe('false');
		expect(field.getAttribute('autocapitalize')).toBe('off');
		expect(field.getAttribute('inputmode')).toBe('text');
	});

	it.each(shapes)('renders none of them when none is given, as %s', (_, multiline) => {
		render(TextInput, { label: 'Handle', value: '', onChange: () => undefined, multiline });
		const field = theField();
		const attributes = field.getAttributeNames().sort();
		expect(attributes).not.toContain('autocomplete');
		expect(attributes).not.toContain('autocorrect');
		expect(attributes).not.toContain('spellcheck');
		expect(attributes).not.toContain('autocapitalize');
		expect(attributes).not.toContain('inputmode');
	});

	it.each(shapes)('renders no name and no id it was not given, as %s', (_, multiline) => {
		const { container } = render(TextInput, {
			label: 'Recovery phrase',
			value: '',
			onChange: () => undefined,
			multiline,
			hint: 'Twelve words',
			error: 'That is too short'
		});
		const field = theField();
		expect(field.hasAttribute('name')).toBe(false);
		expect(field.hasAttribute('id')).toBe(false);
		expect(container.querySelectorAll('input[type="hidden"]')).toHaveLength(0);
	});

	it.each(shapes)('renders a name only when given one, as %s', (_, multiline) => {
		render(TextInput, {
			label: 'Handle',
			value: '',
			onChange: () => undefined,
			multiline,
			name: 'handle'
		});
		expect(theField().getAttribute('name')).toBe('handle');
	});

	it('takes the types a search field and a handle field need', () => {
		render(TextInput, {
			label: 'Search',
			value: '',
			onChange: () => undefined,
			type: 'search',
			inputmode: 'search',
			autocapitalize: 'sentences',
			autocomplete: 'on',
			autocorrect: 'on'
		});
		const field = theField();
		expect(field.getAttribute('type')).toBe('search');
		expect(field.getAttribute('inputmode')).toBe('search');
		expect(field.getAttribute('autocapitalize')).toBe('sentences');
		expect(field.getAttribute('autocomplete')).toBe('on');
		expect(field.getAttribute('autocorrect')).toBe('on');
	});

	it('passes required, disabled, readonly and the placeholder to the field', async () => {
		render(TextInput, {
			label: 'Handle',
			value: '',
			onChange: () => undefined,
			required: true,
			readonly: true,
			placeholder: '@artist.test'
		});
		const field = page.getByRole('textbox', { name: 'Handle' });
		await expect.element(field).toBeRequired();
		await expect.element(field).toHaveAttribute('readonly');
		await expect.element(field).toHaveAttribute('placeholder', '@artist.test');
		render(TextInput, {
			label: 'Display name',
			value: '',
			onChange: () => undefined,
			disabled: true
		});
		await expect.element(page.getByRole('textbox', { name: 'Display name' })).toBeDisabled();
	});

	it('asks nothing while disabled', async () => {
		const onChange = vi.fn<(text: string) => void>();
		render(TextInput, { label: 'Handle', value: '', onChange, disabled: true });
		await userEvent.type(theField(), 'x', { skipClick: true });
		expect(onChange).not.toHaveBeenCalled();
	});

	it('shows a leading icon and a prefix beside the text, the prefix read by no one', async () => {
		const { container } = render(TextInput, {
			label: 'Handle',
			value: 'artist.test',
			onChange: () => undefined,
			icon: 'search',
			prefix: '@',
			mono: true
		});
		expect(container.querySelector('.zf-input__icon svg')).not.toBeNull();
		const prefix = container.querySelector('.zf-input__prefix');
		expect(prefix?.textContent).toBe('@');
		expect(prefix?.getAttribute('aria-hidden')).toBe('true');
		expect(container.querySelector('.zf-input')?.classList.contains('zf-input--mono')).toBe(true);
		await expect.element(page.getByRole('textbox', { name: 'Handle' })).toBeInTheDocument();
	});

	it('puts its test id on the field', async () => {
		render(TextInput, {
			label: 'Handle',
			value: '',
			onChange: () => undefined,
			testid: 'handle-field'
		});
		await expect.element(page.getByTestId('handle-field')).toHaveAttribute('type', 'text');
	});

	it('shows markup in its words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		const { container } = render(TextInput, {
			label: words,
			hint: words,
			error: words,
			placeholder: words,
			value: '',
			onChange: () => undefined
		});
		await expect.element(page.getByRole('textbox', { name: words })).toBeInTheDocument();
		expect(container.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onChange = vi.fn<(text: string) => void>();
		render(TextInput, { label: 'Handle', value: '', onChange });
		await userEvent.type(theField(), 'a');
		expect(onChange).toHaveBeenCalledWith('a');
	});

	it('calls nothing in the seam', async () => {
		const calls: SeamCall[] = [];
		render(
			TextInput,
			{ label: 'Handle', value: '', onChange: () => undefined },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		await userEvent.type(theField(), 'ab');
		expect(calls).toEqual([]);
	});

	it('wears the bundle classes, a slip with the one focus ring on the field', () => {
		const { container } = render(TextInput, {
			label: 'Handle',
			value: '',
			onChange: () => undefined
		});
		const frame = container.querySelector('.zf-input');
		expect(frame?.classList.contains('slip')).toBe(true);
		theField().focus();
		const ring = getComputedStyle(frame ?? document.body);
		expect(ring.outlineStyle).toBe('solid');
		expect(ring.outlineWidth).toBe('2px');
		expect(getComputedStyle(theField()).outlineColor).toBe('rgba(0, 0, 0, 0)');
	});

	it.each([['dark'], ['light']] as const)('takes its colours from the %s skin', (skin) => {
		const scope = inSkin(skin);
		const { container } = render(TextInput, {
			target: scope,
			props: { label: 'Handle', value: '', onChange: () => undefined, error: 'Too short' }
		});
		const frame = container.querySelector('.zf-input') ?? scope;
		const error = container.querySelector('.zf-field__error') ?? scope;
		expect(getComputedStyle(frame).backgroundColor).toBe(tokenColour(scope, '--color-object'));
		expect(getComputedStyle(frame).borderColor).toBe(tokenColour(scope, '--color-danger-text'));
		expect(getComputedStyle(error).color).toBe(tokenColour(scope, '--color-danger-text'));
		scope.remove();
	});
});
