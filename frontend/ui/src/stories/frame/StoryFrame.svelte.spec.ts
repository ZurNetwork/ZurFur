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
});
