import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync, type ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import Toggle from './Toggle.svelte';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The one switch on the page. */
function theSwitch(): HTMLElement {
	const [found] = page.getByRole('switch').elements();
	if (!(found instanceof HTMLElement)) throw new Error('no switch on the page');
	return found;
}

describe('Toggle', () => {
	it('is a switch named by its label and described by its description', async () => {
		render(Toggle, {
			label: 'Open for commissions',
			description: 'New requests land in your queue',
			checked: false,
			onCheckedChange: () => undefined
		});
		const toggle = page.getByRole('switch', { name: 'Open for commissions' });
		await expect.element(toggle).toBeInTheDocument();
		await expect.element(toggle).toHaveAccessibleDescription('New requests land in your queue');
		await expect.element(toggle).toHaveAttribute('aria-checked', 'false');
	});

	it('has no description when none is given', async () => {
		render(Toggle, { label: 'Show my queue', checked: true, onCheckedChange: () => undefined });
		const toggle = page.getByRole('switch', { name: 'Show my queue' });
		await expect.element(toggle).toHaveAttribute('aria-checked', 'true');
		expect(theSwitch().hasAttribute('aria-describedby')).toBe(false);
	});

	it('hands the parent the value a click asks for, as a plain boolean', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Toggle, { label: 'Show my queue', checked: false, onCheckedChange });
		await page.getByRole('switch').click();
		expect(onCheckedChange.mock.calls).toEqual([[true]]);
	});

	it('asks to turn off when it is on', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Toggle, { label: 'Show my queue', checked: true, onCheckedChange });
		await page.getByRole('switch').click();
		expect(onCheckedChange.mock.calls).toEqual([[false]]);
	});

	it('flips with Space and with Enter', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Toggle, { label: 'Show my queue', checked: false, onCheckedChange });
		theSwitch().focus();
		await userEvent.keyboard(' ');
		await userEvent.keyboard('{Enter}');
		expect(onCheckedChange.mock.calls).toEqual([[true], [true]]);
	});

	it('flips when its label is clicked', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Toggle, { label: 'Show my queue', checked: false, onCheckedChange });
		await page.getByText('Show my queue').click();
		expect(onCheckedChange.mock.calls).toEqual([[true]]);
	});

	it('stays off when the parent refuses the change', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Toggle, { label: 'Show my queue', checked: false, onCheckedChange });
		await page.getByRole('switch').click();
		flushSync();
		expect(onCheckedChange).toHaveBeenCalledWith(true);
		await expect.element(page.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
	});

	it('follows the prop when the parent accepts the change', async () => {
		const screen = render(Toggle, {
			label: 'Show my queue',
			checked: false,
			onCheckedChange: () => undefined
		});
		await screen.rerender({ checked: true });
		await expect.element(page.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
	});

	it('asks nothing while disabled', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Toggle, { label: 'Show my queue', checked: false, onCheckedChange, disabled: true });
		const toggle = page.getByRole('switch');
		await expect.element(toggle).toBeDisabled();
		await toggle.click({ force: true });
		theSwitch().focus();
		await userEvent.keyboard(' ');
		expect(onCheckedChange).not.toHaveBeenCalled();
	});

	it('renders a hidden checkbox only when given a name', () => {
		const { container } = render(Toggle, {
			label: 'Show my queue',
			checked: true,
			onCheckedChange: () => undefined
		});
		expect(container.querySelectorAll('input')).toHaveLength(0);
		const named = render(Toggle, {
			label: 'Open for commissions',
			checked: true,
			onCheckedChange: () => undefined,
			name: 'open'
		});
		const inputs = [...named.container.querySelectorAll('input')];
		expect(inputs.map((input) => [input.type, input.name, input.checked])).toEqual([
			['checkbox', 'open', true]
		]);
	});

	it('puts its test id on the switch', async () => {
		render(Toggle, {
			label: 'Show my queue',
			checked: false,
			onCheckedChange: () => undefined,
			testid: 'queue-toggle'
		});
		await expect.element(page.getByTestId('queue-toggle')).toHaveAttribute('role', 'switch');
	});

	it('shows markup in its words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		const { container } = render(Toggle, {
			label: words,
			description: words,
			checked: false,
			onCheckedChange: () => undefined
		});
		await expect.element(page.getByRole('switch', { name: words })).toBeInTheDocument();
		expect(container.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const onCheckedChange = vi.fn<(checked: boolean) => void>();
		render(Toggle, { label: 'Show my queue', checked: false, onCheckedChange });
		await page.getByRole('switch').click();
		expect(onCheckedChange).toHaveBeenCalledWith(true);
	});

	it('squishes its switch and plays the toggle cue on a flip, and nothing else', async () => {
		const calls: SeamCall[] = [];
		render(
			Toggle,
			{ label: 'Show my queue', checked: false, onCheckedChange: () => undefined },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(calls).toEqual([{ kind: 'move', move: 'squish', args: [], element: theSwitch() }]);
		await page.getByRole('switch').click();
		expect(calls.slice(1)).toEqual([{ kind: 'sound', cue: 'toggle' }]);
	});

	it('wears the bundle classes, its thumb placed and gliding', () => {
		render(Toggle, { label: 'Show my queue', checked: false, onCheckedChange: () => undefined });
		const toggle = theSwitch();
		const thumb = toggle.firstElementChild;
		expect(toggle.classList.contains('zf-toggle__track')).toBe(true);
		expect(thumb?.className.split(' ').sort()).toEqual(['glidex', 'placed', 'zf-toggle__thumb']);
	});
});
