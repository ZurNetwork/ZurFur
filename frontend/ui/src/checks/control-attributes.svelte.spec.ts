import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import AcceptingDateField from './testing/AcceptingDateField.svelte';
import AcceptingMenu from './testing/AcceptingMenu.svelte';
import AcceptingPillSwitch from './testing/AcceptingPillSwitch.svelte';
import AcceptingScrollingTabs from './testing/AcceptingScrollingTabs.svelte';
import AcceptingSelect from './testing/AcceptingSelect.svelte';
import AcceptingTabs from './testing/AcceptingTabs.svelte';
import Button from '../lib/controls/Button.svelte';
import Checkbox from '../lib/controls/Checkbox.svelte';
import IconButton from '../lib/controls/IconButton.svelte';
import TextInput from '../lib/controls/TextInput.svelte';
import Toggle from '../lib/controls/Toggle.svelte';
import { isCalendarDay, type CalendarDay } from '../lib/controls/calendar-day.ts';
import { isLinkHref, type LinkHref } from '../lib/controls/link-href.ts';
import { isLocaleTag, type LocaleTag } from '../lib/controls/locale-tag.ts';
import type { MenuItem, PillOption, SelectOption, TabItem } from '../lib/controls/types.ts';
import '../lib/styles/index.ts';

/** The controls' internal helpers: neither is shipped on its own, and Tooltip is reached through IconButton. */
const INTERNAL: ReadonlySet<string> = new Set(['FieldFrame.svelte', 'Tooltip.svelte']);

/** Every shipped control, by file name, read from the folder so a new one can't be missed. */
const CONTROLS: readonly string[] = Object.keys(import.meta.glob('../lib/controls/*.svelte'))
	.map((file) => file.slice(file.lastIndexOf('/') + 1))
	.filter((file) => !INTERNAL.has(file))
	.sort();

/** One way to render a control, and the interactions after each of which it is checked again. */
interface Fixture {
	readonly label: string;
	/** Renders the control; returns the container it rendered into. */
	readonly render: () => HTMLElement;
	/** Each resolves once its interaction has taken effect, a floating part open included. */
	readonly steps: readonly (readonly [label: string, run: () => Promise<void>])[];
}

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** Every attribute named `on…`, in any letter case, on `roots` or anything inside them. */
function onAttributesIn(roots: readonly Element[]): readonly string[] {
	const elements = new Set<Element>(roots);
	for (const root of roots) {
		for (const element of root.querySelectorAll('*')) elements.add(element);
	}
	return [...elements].flatMap((element) =>
		element
			.getAttributeNames()
			.filter((name) => /^on/iu.test(name))
			.map(
				(name) => `<${element.tagName.toLowerCase()} ${name}="${element.getAttribute(name) ?? ''}">`
			)
	);
}

/** Words for a Button, given as markup already escaped. */
function words(escaped: string) {
	return createRawSnippet(() => ({ render: () => `<span>${escaped}</span>` }));
}

function day(text: string): CalendarDay {
	if (!isCalendarDay(text)) throw new Error(`not a calendar day: ${text}`);
	return text;
}

function localeTag(text: string): LocaleTag {
	if (!isLocaleTag(text)) throw new Error(`not a locale tag: ${text}`);
	return text;
}

function linkHref(text: string): LinkHref {
	if (!isLinkHref(text)) throw new Error(`not a link: ${text}`);
	return text;
}

type Choice = 'one' | 'two';

const selectOptions: readonly SelectOption<Choice>[] = [
	{ value: 'one', label: 'One' },
	{ value: 'two', label: 'Two' }
];
const tabs: readonly TabItem<Choice>[] = [
	{ id: 'one', label: 'One', icon: 'file', count: '2' },
	{ id: 'two', label: 'Two' }
];
const pills: readonly PillOption<Choice>[] = [
	{ id: 'one', label: 'One', icon: 'file', prefix: 'batch:' },
	{ id: 'two', label: 'Two' }
];
const menuItems: readonly MenuItem<Choice>[] = [
	{ id: 'one', label: 'One', icon: 'file' },
	{ id: 'two', label: 'Two', tone: 'danger', disabled: true }
];

/** Opens the menu whose trigger carries `trigger`. */
const openMenuStep = [
	'opened',
	async () => {
		await page.getByTestId('trigger').click();
		await expect.element(page.getByRole('menu')).toBeInTheDocument();
	}
] as const;

/** Highlights the open menu's first row. */
const highlightRowStep = [
	'with a row highlighted',
	async () => {
		await page.getByRole('menuitem', { name: 'One' }).hover();
	}
] as const;

/** Opens the menu whose trigger carries `trigger`, then highlights its first row. */
const menuSteps = [openMenuStep, highlightRowStep] as const;

/** Shows the icon button's tip by `how`, and waits for it. */
async function showTip(how: 'hover' | 'focus'): Promise<void> {
	if (how === 'hover') await page.getByTestId('control').hover();
	else byTestId('control').focus();
	await vi.waitFor(
		() => {
			expect(document.querySelector('.zf-tip')?.textContent).toBe('Copy path');
		},
		{ timeout: 3000 }
	);
}

/** Shows the icon menu trigger's tip by `how`, and waits for it. */
async function showMenuTip(how: 'hover' | 'focus'): Promise<void> {
	if (how === 'hover') await page.getByTestId('trigger').hover();
	else byTestId('trigger').focus();
	await vi.waitFor(
		() => {
			expect(document.querySelector('.zf-tip')?.textContent).toBe('More');
		},
		{ timeout: 3000 }
	);
}

/** Shows the icon menu's tip by hover and by focus, opens the menu with the tip showing, then highlights a row. */
const iconMenuSteps = [
	['its tip shown on hover', () => showMenuTip('hover')],
	['its tip shown on focus', () => showMenuTip('focus')],
	[
		'opened with its tip shown',
		async () => {
			await showMenuTip('hover');
			await openMenuStep[1]();
		}
	],
	highlightRowStep
] as const;

const FIXTURES: Readonly<Record<string, readonly Fixture[]>> = {
	'Button.svelte': [
		{
			label: 'as a button',
			render: () =>
				render(Button, {
					children: words('Send'),
					onPress: () => undefined,
					icon: 'file',
					iconAfter: 'copy',
					testid: 'control'
				}).container,
			steps: [['pressed', () => page.getByTestId('control').click()]]
		},
		{
			label: 'as a link',
			render: () =>
				render(Button, { children: words('Open'), href: linkHref('/den'), testid: 'control' })
					.container,
			steps: [['hovered', () => page.getByTestId('control').hover()]]
		}
	],
	'Checkbox.svelte': [
		{
			label: 'with a description and a name',
			render: () =>
				render(Checkbox, {
					label: 'Agree',
					description: 'To the terms',
					checked: 'mixed',
					onCheckedChange: () => undefined,
					name: 'agree',
					value: 'yes',
					testid: 'control'
				}).container,
			steps: [['pressed by its label', () => page.getByText('Agree').click()]]
		}
	],
	'DateField.svelte': [
		{
			label: 'with bounds and a name',
			render: () =>
				render(AcceptingDateField, {
					label: 'Due',
					locale: localeTag('de-DE'),
					words: { day: 'Tag', month: 'Monat', year: 'Jahr', empty: 'leer' },
					initial: day('2026-03-04'),
					min: day('2026-01-01'),
					max: day('2026-12-31'),
					name: 'due',
					onChange: () => undefined
				}).container,
			steps: [
				[
					'edited',
					async () => {
						await page.getByRole('spinbutton', { name: 'Tag' }).click();
						await userEvent.keyboard('{ArrowUp}');
					}
				]
			]
		}
	],
	'IconButton.svelte': [
		{
			label: 'with its tip',
			render: () =>
				render(IconButton, {
					icon: 'copy',
					label: 'Copy path',
					onPress: () => undefined,
					pressed: true,
					expanded: false,
					controls: 'list',
					testid: 'control'
				}).container,
			steps: [
				['its tip shown on hover', () => showTip('hover')],
				['its tip shown on focus', () => showTip('focus')],
				['pressed', () => page.getByTestId('control').click()]
			]
		}
	],
	'Menu.svelte': [
		{
			label: 'with a button trigger',
			render: () =>
				render(AcceptingMenu<Choice>, {
					trigger: { kind: 'button', label: 'Actions', icon: 'file' },
					items: menuItems,
					testid: 'trigger',
					onOpenChange: () => undefined,
					onSelect: () => undefined
				}).container,
			steps: menuSteps
		},
		{
			label: 'with an icon trigger',
			render: () =>
				render(AcceptingMenu<Choice>, {
					trigger: { kind: 'icon', label: 'More', icon: 'ellipsis' },
					items: menuItems,
					testid: 'trigger',
					onOpenChange: () => undefined,
					onSelect: () => undefined
				}).container,
			steps: iconMenuSteps
		}
	],
	'PillSwitch.svelte': [
		{
			label: 'with a name',
			render: () =>
				render(AcceptingPillSwitch<Choice>, {
					label: 'Batch',
					options: pills,
					initial: 'one',
					accepts: true,
					name: 'batch',
					testid: 'control',
					onChange: () => undefined
				}).container,
			steps: [['chosen', () => page.getByRole('radio', { name: 'Two' }).click()]]
		}
	],
	'Select.svelte': [
		{
			label: 'with a placeholder, hint, error and name',
			render: () =>
				render(AcceptingSelect<Choice>, {
					label: 'Pick',
					options: selectOptions,
					initial: undefined,
					placeholder: 'Choose one',
					hint: 'A hint',
					error: 'An error',
					name: 'pick',
					testid: 'control',
					onChange: () => undefined
				}).container,
			steps: [
				[
					'opened',
					async () => {
						await page.getByTestId('control').click();
						await expect.element(page.getByRole('listbox')).toBeInTheDocument();
					}
				],
				[
					'with an option highlighted',
					async () => {
						await page.getByRole('option', { name: 'Two' }).hover();
						await expect
							.element(page.getByRole('option', { name: 'Two' }))
							.toHaveAttribute('data-highlighted');
					}
				],
				[
					'chosen',
					async () => {
						await page.getByRole('option', { name: 'Two' }).click();
						await expect.element(page.getByTestId('control')).toHaveTextContent('Two');
					}
				]
			]
		}
	],
	'Tabs.svelte': [
		{
			label: 'with a panel',
			render: () =>
				render(AcceptingTabs<Choice>, {
					label: 'Sections',
					tabs,
					initial: 'one',
					accepts: true,
					testid: 'control',
					onChange: () => undefined
				}).container,
			steps: [['switched', () => page.getByRole('tab', { name: 'Two' }).click()]]
		}
	],
	'ScrollingTabs.svelte': [
		{
			label: 'narrow, with a panel and its step buttons',
			render: () => {
				// Narrow enough that the tabs outgrow their list, so the forward step can be pressed.
				const narrow = document.createElement('div');
				narrow.style.width = '120px';
				document.body.append(narrow);
				return render(AcceptingScrollingTabs<Choice>, {
					target: narrow,
					props: {
						label: 'Sections',
						tabs,
						initial: 'one',
						accepts: true,
						stepWords: { back: 'Earlier tabs', forward: 'More tabs' },
						testid: 'control',
						onChange: () => undefined
					}
				}).container;
			},
			steps: [
				['stepped', () => page.getByRole('button', { name: 'More tabs' }).click()],
				['switched', () => page.getByRole('tab', { name: 'Two' }).click()]
			]
		}
	],
	'TextInput.svelte': [
		{
			label: 'on one line, with every part',
			render: () =>
				render(TextInput, {
					label: 'Handle',
					value: '',
					onChange: () => undefined,
					hint: 'A hint',
					error: 'An error',
					icon: 'file',
					prefix: '@',
					name: 'handle',
					placeholder: 'you',
					testid: 'control'
				}).container,
			steps: [['typed in', () => page.getByTestId('control').fill('zuri')]]
		},
		{
			label: 'multiline',
			render: () =>
				render(TextInput, {
					label: 'Notes',
					value: '',
					onChange: () => undefined,
					multiline: true,
					testid: 'control'
				}).container,
			steps: [['typed in', () => page.getByTestId('control').fill('notes')]]
		}
	],
	'Toggle.svelte': [
		{
			label: 'with a description and a name',
			render: () =>
				render(Toggle, {
					label: 'Open for commissions',
					description: 'Shown on the profile',
					checked: false,
					onCheckedChange: () => undefined,
					name: 'open',
					testid: 'control'
				}).container,
			steps: [['flipped', () => page.getByTestId('control').click()]]
		}
	]
};

type Case = readonly [label: string, fixture: Fixture | undefined];

/** Every case to run: one per fixture, or a missing one for a control with none. */
const CASES: readonly Case[] = CONTROLS.flatMap((control): readonly Case[] => {
	const fixtures = FIXTURES[control] ?? [];
	if (fixtures.length === 0) return [[control, undefined]];
	return fixtures.map((fixture): Case => [`${control} ${fixture.label}`, fixture]);
});

describe('what a control renders', () => {
	it('finds the controls', () => {
		expect(CONTROLS).toContain('IconButton.svelte');
		expect(CONTROLS).not.toContain('Tooltip.svelte');
	});

	it('has fixtures only for controls that exist', () => {
		expect(Object.keys(FIXTURES).filter((control) => !CONTROLS.includes(control))).toEqual([]);
	});

	it('finds an on… attribute in any letter case, in the body', () => {
		const planted = document.createElement('div');
		planted.setAttribute('OnClick', 'x');
		document.body.append(planted);
		expect(onAttributesIn([document.body])).toEqual(['<div onclick="x">']);
		planted.remove();
	});

	it.each(CASES)('%s carries no on… attribute', async (_label, fixture) => {
		expect(fixture, 'every control needs a fixture here').toBeDefined();
		if (fixture === undefined) return;
		const container = fixture.render();
		const roots = [container, document.body];
		expect(container.querySelectorAll('*').length).toBeGreaterThan(0);
		const seen: (readonly [moment: string, found: readonly string[]])[] = [
			['at rest', onAttributesIn(roots)]
		];
		for (const [label, run] of fixture.steps) {
			await run();
			seen.push([label, onAttributesIn(roots)]);
		}
		expect(seen.filter(([, found]) => found.length > 0)).toEqual([]);
	});
});
