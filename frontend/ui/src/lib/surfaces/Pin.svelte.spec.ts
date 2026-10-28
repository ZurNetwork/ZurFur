import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import Pin from './Pin.svelte';
import { pinPointOf, type PinPoint } from './pin-point.ts';
import { contrastRatio, parseColor, type Rgba } from '../../gallery/contrast/contrast.ts';
import { SKIN_ATTRIBUTE, type Skin } from '../styles/environment.ts';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** A point minted the one way. */
function pointAt(x: number, y: number): PinPoint {
	const point = pinPointOf(x, y);
	if (point === undefined) throw new Error(`(${String(x)}, ${String(y)}) is not a point`);
	return point;
}

/** A pin's props, with a spy for its press. */
function pinProps(overrides: Partial<ComponentProps<typeof Pin>> = {}): ComponentProps<typeof Pin> {
	const props: ComponentProps<typeof Pin> = {
		point: pointAt(0.5, 0.25),
		mark: '3',
		label: 'Pin 3: the left ear',
		open: false,
		onPress: vi.fn(),
		testid: 'pin',
		...overrides
	};
	return props;
}

/** Points no pin may sit on, cast in as a page could still hand them over. */
const badPoints: readonly unknown[] = [
	{ x: 1.0001, y: 0.5 },
	{ x: 0.5, y: -0.0001 },
	{ x: Number.NaN, y: 0.5 },
	{ x: 0.5, y: Number.POSITIVE_INFINITY },
	{ x: '50%', y: 0.5 },
	{ x: 'calc(1px)', y: 0.5 },
	{ x: 0.5 },
	undefined,
	'0.5,0.5',
	[]
];

describe('Pin', () => {
	it('is a button named by its label, showing its mark', async () => {
		render(Pin, pinProps());
		const pin = page.getByRole('button', { name: 'Pin 3: the left ear' });
		await expect.element(pin).toBeInTheDocument();
		await expect.element(pin).toHaveTextContent('3');
		await expect.element(pin).toHaveAttribute('type', 'button');
	});

	it('takes its left and top from the point, as percentages', () => {
		render(Pin, pinProps({ point: pointAt(0.5, 0.25) }));
		const pin = byTestId('pin');
		expect(pin.style.left).toBe('50%');
		expect(pin.style.top).toBe('25%');
		expect(pin.getAttribute('style')).toBe('left: 50%; top: 25%;');
	});

	it.each(badPoints)('draws nothing for the point %o', (point) => {
		const { container } = render(Pin, pinProps({ point: point as PinPoint }));
		expect(container.querySelector('button')).toBeNull();
		expect(container.textContent.trim()).toBe('');
	});

	it('records the squish on the button, and nothing else', () => {
		const calls: SeamCall[] = [];
		render(Pin, pinProps(), {
			wrapper: RecordingEnvironment,
			wrapperProps: { onRecord: (call) => calls.push(call) }
		});
		expect(calls).toEqual([{ kind: 'move', move: 'squish', args: [], element: byTestId('pin') }]);
	});

	it('calls onPress once on a click', async () => {
		const onPress = vi.fn();
		render(Pin, pinProps({ onPress }));
		await page.getByRole('button').click();
		expect(onPress).toHaveBeenCalledTimes(1);
		expect(onPress).toHaveBeenCalledWith();
	});

	it.each(['{Enter}', ' '])('calls onPress once on %j', async (key) => {
		const onPress = vi.fn();
		render(Pin, pinProps({ onPress }));
		byTestId('pin').focus();
		await userEvent.keyboard(key);
		expect(onPress).toHaveBeenCalledTimes(1);
	});

	it('is reached by Tab', async () => {
		render(Pin, pinProps());
		await userEvent.tab();
		expect(document.activeElement).toBe(byTestId('pin'));
	});

	it('when open, says it is expanded and wears is-active', () => {
		render(Pin, pinProps({ open: true }));
		const pin = byTestId('pin');
		expect(pin.getAttribute('aria-expanded')).toBe('true');
		expect(pin.classList.contains('is-active')).toBe(true);
	});

	it('when closed, says it is collapsed and is not active', () => {
		render(Pin, pinProps({ open: false }));
		const pin = byTestId('pin');
		expect(pin.getAttribute('aria-expanded')).toBe('false');
		expect(pin.classList.contains('is-active')).toBe(false);
	});

	it.each(['true', 1, {}])('stays closed for the open value %o', (stranger) => {
		render(Pin, pinProps({ open: stranger as boolean }));
		const pin = byTestId('pin');
		expect(pin.getAttribute('aria-expanded')).toBe('false');
		expect(pin.classList.contains('is-active')).toBe(false);
	});

	it('names the thread it opens when given its id', () => {
		render(Pin, pinProps({ controls: 'thread-3' }));
		expect(byTestId('pin').getAttribute('aria-controls')).toBe('thread-3');
	});

	it('names no thread when given none', () => {
		render(Pin, pinProps());
		expect(byTestId('pin').hasAttribute('aria-controls')).toBe(false);
	});

	it('shows its mark as text, never markup', () => {
		render(Pin, pinProps({ mark: '<b>3</b>' }));
		const pin = byTestId('pin');
		expect(pin.querySelector('b')).toBeNull();
		expect(pin.textContent.trim()).toBe('<b>3</b>');
	});

	it('shows a mark and a label written as an image tag as text, with no image', () => {
		const markup = '<img src=x onerror=alert(1)>';
		const { container } = render(Pin, pinProps({ mark: markup, label: markup }));
		const pin = byTestId('pin');
		expect(container.querySelector('img')).toBeNull();
		expect(pin.textContent.trim()).toBe(markup);
		expect(pin.getAttribute('aria-label')).toBe(markup);
	});

	it('shows no word of its own', () => {
		const { container } = render(Pin, pinProps());
		expect(container.textContent.trim()).toBe('3');
	});
});

/** The look a pin wears around its edge: its outline and its rings of shadow. */
interface EdgeLook {
	readonly outlineStyle: string;
	readonly outlineWidth: string;
	readonly outlineColor: string;
	readonly outlineOffset: string;
	readonly boxShadow: string;
}

function edgeLookOf(element: Element): EdgeLook {
	const style = getComputedStyle(element);
	return {
		outlineStyle: style.outlineStyle,
		outlineWidth: style.outlineWidth,
		outlineColor: style.outlineColor,
		outlineOffset: style.outlineOffset,
		boxShadow: style.boxShadow
	};
}

/** One computed shadow ring: its colour and how far it spreads past the edge, in pixels. */
interface Band {
	readonly color: Rgba;
	readonly spread: number;
}

/** A colour read from computed text, or a failed test. */
function colorOf(text: string): Rgba {
	const color = parseColor(text);
	if (color.kind !== 'read') throw new Error(color.reason);
	return color.value;
}

/** The token colour as the page resolves it on `element`. */
function tokenColor(element: Element, name: string): Rgba {
	const probe = document.createElement('span');
	probe.style.color = `var(${name})`;
	element.append(probe);
	const color = colorOf(getComputedStyle(probe).color);
	probe.remove();
	return color;
}

/** The computed `box-shadow` as rings, each a colour and a spread with no offset or blur. */
function bandsOf(boxShadow: string): readonly Band[] {
	if (boxShadow === 'none') return [];
	const layers = boxShadow.split(/,(?![^(]*\))/).map((layer) => layer.trim());
	return layers.map((layer) => {
		const match = /^(rgba?\([^)]*\)) 0px 0px 0px (\d+(?:\.\d+)?)px$/.exec(layer);
		if (match === null) throw new Error(`${layer} is not a plain ring`);
		const [, color = '', spread = ''] = match;
		return { color: colorOf(color), spread: Number(spread) };
	});
}

function pixels(text: string): number {
	return Number(text.replace(/px$/, ''));
}

describe('Pin focused by keyboard', () => {
	afterEach(() => {
		document.documentElement.removeAttribute(SKIN_ATTRIBUTE);
	});

	it.each<Skin>(['dark', 'light'])(
		'in the %s skin, rings itself in its own two colours, at least 9:1 apart, outside its gold',
		async (skin) => {
			document.documentElement.setAttribute(SKIN_ATTRIBUTE, skin);
			render(Pin, pinProps());
			await userEvent.tab();
			const pin = byTestId('pin');
			expect(pin.matches(':focus-visible')).toBe(true);
			const look = edgeLookOf(pin);
			const ring = tokenColor(pin, '--color-pin-ring');
			const halo = tokenColor(pin, '--color-pin-halo');

			expect(look.outlineStyle).toBe('solid');
			expect(pixels(look.outlineWidth)).toBeGreaterThanOrEqual(2);
			expect(colorOf(look.outlineColor)).toEqual(ring);

			const bands = [...bandsOf(look.boxShadow)].sort((one, other) => one.spread - other.spread);
			const outer = bands.at(-1);
			const under = bands.at(-2);
			expect(outer).toBeDefined();
			expect(under).toBeDefined();
			if (outer === undefined || under === undefined) return;
			expect(outer.color).toEqual(halo);
			expect(outer.spread - under.spread).toBeGreaterThanOrEqual(2);
			expect(pixels(look.outlineOffset)).toBe(outer.spread);
			expect(contrastRatio(colorOf(look.outlineColor), outer.color)).toBeGreaterThanOrEqual(9);
			expect(under.color).toEqual(tokenColor(pin, '--color-focus'));
		}
	);

	it('looks unlike the resting pin and unlike the open one', async () => {
		render(Pin, pinProps({ testid: 'resting', label: 'Resting' }));
		render(Pin, pinProps({ testid: 'open', label: 'Open', open: true }));
		render(Pin, pinProps({ testid: 'focused', label: 'Focused' }));
		await userEvent.tab();
		await userEvent.tab();
		await userEvent.tab();
		const focused = byTestId('focused');
		expect(focused.matches(':focus-visible')).toBe(true);
		const look = edgeLookOf(focused);
		expect(look).not.toEqual(edgeLookOf(byTestId('resting')));
		expect(look).not.toEqual(edgeLookOf(byTestId('open')));
		expect(look.boxShadow).not.toBe(edgeLookOf(byTestId('open')).boxShadow);
	});
});
