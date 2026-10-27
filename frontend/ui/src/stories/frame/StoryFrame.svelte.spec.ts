import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import StoryFrame from './StoryFrame.svelte';
import type { SkinChoice } from './globals.ts';
import type { MotionMode } from '../../lib/styles/environment.ts';
import '../../lib/styles/index.ts';

/** A story that prints a word to find it by. */
const children = createRawSnippet(() => ({
	render: () => '<p class="the-story">the story</p>'
}));

const root = document.documentElement;

afterEach(() => {
	root.removeAttribute('data-skin');
	root.removeAttribute('data-motion');
});

/** The skin and motion attributes on `<html>` and on the story's wrapper. */
function attributes(): {
	readonly root: readonly (string | null)[];
	readonly wrapper: readonly (string | null)[] | undefined;
} {
	const story = document.querySelector('.the-story');
	const wrapper = story?.parentElement;
	return {
		root: [root.getAttribute('data-skin'), root.getAttribute('data-motion')],
		wrapper:
			wrapper === null || wrapper === undefined
				? undefined
				: [wrapper.getAttribute('data-skin'), wrapper.getAttribute('data-motion')]
	};
}

/** The skin the page shows when it follows the OS, as the tokens read the OS. */
function osSkin(): string {
	return matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

function frame(skin: SkinChoice, motion: MotionMode) {
	return render(StoryFrame, { skin, motion, children });
}

/** A story of a fixed layer, pinned to every edge of whatever contains it. */
const fixedLayer = createRawSnippet(() => ({
	render: () => '<div class="the-layer" style="position: fixed; inset: 0"></div>'
}));

/** The containment and minimum height of the frame's wrapper, and where its fixed layer sits. */
function layerShape(layer: boolean | undefined) {
	const props = layer === undefined ? {} : { layer };
	render(StoryFrame, { skin: 'dark', motion: 'lively', children: fixedLayer, ...props });
	const wrapper = document.querySelector('.story-frame');
	const fixed = document.querySelector('.the-layer');
	if (wrapper === null || fixed === null) throw new Error('the frame or its layer is missing');
	const style = getComputedStyle(wrapper);
	const box = (element: Element) => {
		const { top, left, width, height } = element.getBoundingClientRect();
		return { top, left, width, height };
	};
	return {
		contain: style.contain,
		minHeight: style.minHeight,
		layerBox: box(fixed),
		wrapperBox: box(wrapper),
		skinHolder: fixed.closest('[data-skin]') === wrapper
	};
}

describe('StoryFrame', () => {
	it('renders the story inside its wrapper', () => {
		frame('dark', 'lively');
		const story = document.querySelector('.the-story');
		expect(story?.textContent).toBe('the story');
		expect(story?.parentElement?.classList.contains('story-frame')).toBe(true);
	});

	it.each([
		[
			'dark',
			'lively',
			[
				['dark', null],
				['dark', null]
			]
		],
		[
			'light',
			'lively',
			[
				['light', null],
				['light', null]
			]
		],
		[
			'dark',
			'still',
			[
				['dark', 'still'],
				['dark', 'still']
			]
		],
		[
			'light',
			'still',
			[
				['light', 'still'],
				['light', 'still']
			]
		]
	] as const)('puts %s and %s on <html> and the wrapper', (skin, motion, [onRoot, onWrapper]) => {
		frame(skin, motion);
		expect(attributes()).toEqual({ root: onRoot, wrapper: onWrapper });
	});

	it.each(['lively', 'still'] as const)(
		'leaves <html> without a skin under Follow the OS, and gives the wrapper the OS skin (%s)',
		(motion) => {
			frame('os', motion);
			const motionAttribute = motion === 'still' ? 'still' : null;
			expect(attributes()).toEqual({
				root: [null, motionAttribute],
				wrapper: [osSkin(), motionAttribute]
			});
		}
	);

	it('takes the skin off <html> when the choice goes back to Follow the OS', async () => {
		const screen = frame('light', 'still');
		expect(attributes().root).toEqual(['light', 'still']);
		await screen.rerender({ skin: 'os', motion: 'lively' });
		expect(attributes()).toEqual({ root: [null, null], wrapper: [osSkin(), null] });
	});

	it('moves <html> from one skin to the other', async () => {
		const screen = frame('light', 'lively');
		await screen.rerender({ skin: 'dark' });
		expect(attributes().root).toEqual(['dark', null]);
	});

	it('takes both attributes off <html> when it goes away', async () => {
		const screen = frame('dark', 'still');
		await screen.unmount();
		expect(attributes().root).toEqual([null, null]);
	});

	it('paints the wrapper in the skin it names', () => {
		frame('light', 'lively');
		const wrapper = document.querySelector('.story-frame');
		expect(wrapper === null ? undefined : getComputedStyle(wrapper).backgroundColor).toBe(
			'rgb(236, 229, 211)'
		);
	});

	it.each([
		['light', 'rgb(236, 229, 211)', 'rgb(45, 43, 43)'],
		['dark', 'rgb(21, 20, 23)', 'rgb(219, 219, 219)']
	] as const)(
		'paints a docs page around it in %s, its title included',
		(skin, background, text) => {
			const docs = document.createElement('div');
			docs.className = 'sbdocs sbdocs-wrapper';
			const content = document.createElement('div');
			content.className = 'sbdocs sbdocs-content';
			const title = document.createElement('h1');
			title.textContent = 'Colour';
			content.append(title);
			docs.append(content);
			document.body.append(docs);
			try {
				frame(skin, 'lively');
				expect({
					background: getComputedStyle(docs).backgroundColor,
					title: getComputedStyle(title).color
				}).toEqual({ background, title: text });
			} finally {
				docs.remove();
			}
		}
	);

	it.each([
		['by default', undefined],
		['when layer is false', false]
	] as const)('leaves the wrapper uncontained %s', (_label, layer) => {
		const shape = layerShape(layer);
		expect({ contain: shape.contain, minHeight: shape.minHeight }).toEqual({
			contain: 'none',
			minHeight: '0px'
		});
		expect(shape.layerBox).toEqual({
			top: 0,
			left: 0,
			width: document.documentElement.clientWidth,
			height: window.innerHeight
		});
	});

	it('holds a fixed layer inside the wrapper when layer is true, under its skin', () => {
		const shape = layerShape(true);
		expect({ contain: shape.contain, minHeight: shape.minHeight }).toEqual({
			contain: 'layout paint',
			minHeight: '480px'
		});
		expect(shape.layerBox).toEqual(shape.wrapperBox);
		expect(shape.wrapperBox.height).toBe(480);
		expect(shape.skinHolder).toBe(true);
	});
});
