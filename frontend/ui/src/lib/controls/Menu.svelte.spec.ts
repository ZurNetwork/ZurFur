import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync, type ComponentProps } from 'svelte';
import AcceptingMenu from '../../checks/testing/AcceptingMenu.svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import Menu from './Menu.svelte';
import type { MenuItem, MenuTrigger } from './types.ts';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

type Action = 'rename' | 'duplicate' | 'archive' | 'delete';

const items: readonly MenuItem<Action>[] = [
	{ id: 'rename', label: 'Rename', icon: 'file' },
	{ id: 'duplicate', label: 'Duplicate', icon: 'copy' },
	{ id: 'archive', label: 'Move to archive', icon: 'archive', disabled: true },
	{ id: 'delete', label: 'Delete commission', tone: 'danger' }
];

const actions: MenuTrigger = { kind: 'button', label: 'Actions' };
const more: MenuTrigger = { kind: 'icon', label: 'More actions', icon: 'ellipsis' };

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** The open menu's element, wherever it portalled, or undefined while it is closed. */
function openMenu(): HTMLElement | undefined {
	const [found] = page.getByRole('menu').elements();
	return found instanceof HTMLElement ? found : undefined;
}

/** The menu item named `name`. */
function menuItem(name: string): HTMLElement {
	const [found] = page.getByRole('menuitem', { name }).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`no menu item ${name}`);
	return found;
}

interface Spies {
	readonly onOpenChange: ReturnType<typeof vi.fn<(open: boolean) => void>>;
	readonly onSelect: ReturnType<typeof vi.fn<(id: Action) => void>>;
}

/** A menu whose parent accepts every open and close, as most parents do. */
function renderAccepting(
	trigger: MenuTrigger = actions,
	target: HTMLElement = document.body,
	wrapped?: (call: SeamCall) => void
): Spies {
	const spies: Spies = { onOpenChange: vi.fn(), onSelect: vi.fn() };
	const props = {
		trigger,
		items,
		testid: 'trigger',
		onOpenChange: spies.onOpenChange,
		onSelect: spies.onSelect
	};
	if (wrapped === undefined) {
		render(AcceptingMenu<Action>, { target, props });
	} else {
		const recording = { wrapper: RecordingEnvironment, wrapperProps: { onRecord: wrapped } };
		render(AcceptingMenu<Action>, { target, props }, recording);
	}
	return spies;
}

afterEach(() => {
	document.documentElement.removeAttribute('data-skin');
	for (const copy of document.querySelectorAll('[data-copy]')) copy.remove();
});

describe('Menu', () => {
	it('is a closed menu button named by its label', async () => {
		renderAccepting();
		const trigger = page.getByRole('button', { name: 'Actions' });
		await expect.element(trigger).toHaveAttribute('aria-haspopup', 'menu');
		await expect.element(trigger).toHaveAttribute('aria-expanded', 'false');
		expect(openMenu()).toBeUndefined();
	});

	it('names an icon trigger by its label', async () => {
		renderAccepting(more);
		const trigger = page.getByRole('button', { name: 'More actions' });
		await expect.element(trigger).toHaveAttribute('aria-haspopup', 'menu');
		expect(byTestId('trigger').classList.contains('zf-iconbtn')).toBe(true);
	});

	it('opens on a click, asking with a plain true, and shows every item in order', async () => {
		const { onOpenChange } = renderAccepting();
		await page.getByRole('button', { name: 'Actions' }).click();
		await expect.element(page.getByRole('menu')).toBeInTheDocument();
		expect(onOpenChange.mock.calls).toEqual([[true]]);
		const names = page
			.getByRole('menuitem')
			.elements()
			.map((item) => item.textContent.trim());
		expect(names).toEqual(['Rename', 'Duplicate', 'Move to archive', 'Delete commission']);
		await expect
			.element(page.getByRole('button', { name: 'Actions' }))
			.toHaveAttribute('aria-expanded', 'true');
	});

	it.each([
		['Enter', '{Enter}'],
		['Space', ' '],
		['the down arrow', '{ArrowDown}']
	])('opens on %s, onto its first item', async (_, key) => {
		renderAccepting();
		byTestId('trigger').focus();
		await userEvent.keyboard(key);
		await expect.element(page.getByRole('menu')).toBeInTheDocument();
		await vi.waitFor(() => {
			expect(document.activeElement).toBe(menuItem('Rename'));
		});
	});

	it('moves with the arrows, skips a disabled item, finds by typing and chooses on Enter', async () => {
		const { onSelect, onOpenChange } = renderAccepting();
		byTestId('trigger').focus();
		await userEvent.keyboard('{ArrowDown}');
		await vi.waitFor(() => {
			expect(document.activeElement).toBe(menuItem('Rename'));
		});
		await userEvent.keyboard('{ArrowDown}');
		expect(document.activeElement).toBe(menuItem('Duplicate'));
		await userEvent.keyboard('{ArrowDown}');
		expect(document.activeElement).toBe(menuItem('Delete commission'));
		await userEvent.keyboard('r');
		expect(document.activeElement).toBe(menuItem('Rename'));
		await userEvent.keyboard('{Enter}');
		expect(onSelect.mock.calls).toEqual([['rename']]);
		expect(onOpenChange.mock.calls).toEqual([[true], [false]]);
		await vi.waitFor(() => {
			expect(openMenu()).toBeUndefined();
		});
		expect(document.activeElement).toBe(byTestId('trigger'));
	});

	it('hands back the typed id of the item clicked', async () => {
		const { onSelect } = renderAccepting();
		await page.getByRole('button', { name: 'Actions' }).click();
		await page.getByRole('menuitem', { name: 'Delete commission' }).click();
		expect(onSelect.mock.calls).toEqual([['delete']]);
	});

	it('closes on Escape and returns focus to its trigger', async () => {
		const { onOpenChange, onSelect } = renderAccepting();
		byTestId('trigger').focus();
		await userEvent.keyboard('{Enter}');
		await expect.element(page.getByRole('menu')).toBeInTheDocument();
		await userEvent.keyboard('{Escape}');
		await vi.waitFor(() => {
			expect(openMenu()).toBeUndefined();
		});
		expect(onOpenChange.mock.calls).toEqual([[true], [false]]);
		expect(onSelect).not.toHaveBeenCalled();
		expect(document.activeElement).toBe(byTestId('trigger'));
	});

	it('never chooses a disabled item', async () => {
		const { onSelect } = renderAccepting();
		await page.getByRole('button', { name: 'Actions' }).click();
		const archive = page.getByRole('menuitem', { name: 'Move to archive' });
		await expect.element(archive).toHaveAttribute('aria-disabled', 'true');
		await archive.click({ force: true });
		menuItem('Move to archive').focus();
		await userEvent.keyboard('{Enter}');
		expect(onSelect).not.toHaveBeenCalled();
		expect(openMenu()).toBeDefined();
	});

	it('stays closed when the parent refuses to open it', async () => {
		const onOpenChange = vi.fn<(open: boolean) => void>();
		render(Menu<Action>, {
			trigger: actions,
			items,
			open: false,
			onOpenChange,
			onSelect: () => undefined
		});
		await page.getByRole('button', { name: 'Actions' }).click();
		flushSync();
		expect(onOpenChange).toHaveBeenCalledWith(true);
		await expect
			.element(page.getByRole('button', { name: 'Actions' }))
			.toHaveAttribute('aria-expanded', 'false');
		expect(openMenu()).toBeUndefined();
	});

	it('shows markup in its words as text', async () => {
		const words = '<img src=x onerror=alert(1)>';
		render(AcceptingMenu<'markup'>, {
			trigger: { kind: 'button', label: words },
			items: [{ id: 'markup', label: words }],
			onOpenChange: () => undefined,
			onSelect: () => undefined
		});
		await page.getByRole('button', { name: words }).click();
		await expect.element(page.getByRole('menuitem', { name: words })).toBeInTheDocument();
		expect(document.querySelector('img')).toBeNull();
	});

	it('works with no environment provided', async () => {
		const { onSelect } = renderAccepting();
		await page.getByRole('button', { name: 'Actions' }).click();
		await page.getByRole('menuitem', { name: 'Duplicate' }).click();
		expect(onSelect.mock.calls).toEqual([['duplicate']]);
	});

	it('squishes its trigger, puts the menu down as a note in place, and cues the open and the close', async () => {
		const calls: SeamCall[] = [];
		renderAccepting(actions, document.body, (call) => calls.push(call));
		expect(calls).toEqual([
			{ kind: 'move', move: 'squish', args: [], element: byTestId('trigger') }
		]);
		await page.getByRole('button', { name: 'Actions' }).click();
		await expect.element(page.getByRole('menu')).toBeInTheDocument();
		const menu = openMenu();
		expect(calls.slice(1)).toEqual([
			{ kind: 'sound', cue: 'open' },
			{ kind: 'move', move: 'note', args: ['in-place'], element: menu }
		]);
		await userEvent.keyboard('{Escape}');
		await vi.waitFor(() => {
			expect(openMenu()).toBeUndefined();
		});
		expect(calls.slice(3)).toEqual([{ kind: 'sound', cue: 'close' }]);
	});

	it('wears the note and pick rows, a danger row in the danger ink', async () => {
		renderAccepting();
		await page.getByRole('button', { name: 'Actions' }).click();
		await expect.element(page.getByRole('menu')).toHaveClass('zf-menu');
		expect(menuItem('Rename').className.split(' ').sort()).toEqual(['pick', 'zf-menu__item']);
		expect(menuItem('Delete commission').classList.contains('zf-menu__item--danger')).toBe(true);
		const danger = getComputedStyle(menuItem('Delete commission')).color;
		expect(danger).not.toBe(getComputedStyle(menuItem('Rename')).color);
		expect(byTestId('trigger').className.split(' ').sort()).toEqual(
			['placed', 'zf-btn', 'zf-btn--secondary'].sort()
		);
	});

	it.each([
		['light', 'dark', 'rgb(252, 247, 234)'],
		['dark', 'light', 'rgb(39, 37, 41)']
	])(
		'opens inside a %s copy on a %s page, on that copy’s note colour',
		async (skin, pageSkin, note) => {
			document.documentElement.setAttribute('data-skin', pageSkin);
			const copy = document.createElement('div');
			copy.setAttribute('data-skin', skin);
			copy.setAttribute('data-copy', '');
			document.body.append(copy);
			renderAccepting(actions, copy);
			await page.getByRole('button', { name: 'Actions' }).click();
			await expect.element(page.getByRole('menu')).toBeInTheDocument();
			const menu = openMenu();
			expect(menu?.closest('[data-skin]')).toBe(copy);
			expect(menu === undefined ? '' : getComputedStyle(menu).backgroundColor).toBe(note);
		}
	);

	it('opens into the body when the page itself carries the skin', async () => {
		document.documentElement.setAttribute('data-skin', 'light');
		renderAccepting();
		await page.getByRole('button', { name: 'Actions' }).click();
		await expect.element(page.getByRole('menu')).toBeInTheDocument();
		expect(openMenu()?.parentElement?.parentElement).toBe(document.body);
	});
});
