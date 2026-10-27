/**
 * The preview: the design tokens, the Skin and Motion toolbar menus, the
 * frame every story renders in, the accessibility rules every story is held
 * to, and the Still check.
 */
import type { Preview } from '@storybook/sveltekit';
import { expect } from 'storybook/test';
import '@zurfur/ui/styles';
import StoryFrame from '../src/stories/frame/StoryFrame.svelte';
import { parseMotion, parseSkinChoice } from '../src/stories/frame/globals.ts';

const preview: Preview = {
	globalTypes: {
		skin: {
			description: 'Skin',
			toolbar: {
				title: 'Skin',
				icon: 'contrast',
				items: [
					{ value: 'os', title: 'Follow the OS' },
					{ value: 'dark', title: 'Paper dark' },
					{ value: 'light', title: 'Paper light' }
				],
				dynamicTitle: true
			}
		},
		motion: {
			description: 'Motion',
			toolbar: {
				title: 'Motion',
				icon: 'lightning',
				items: [
					{ value: 'lively', title: 'Lively' },
					{ value: 'still', title: 'Still' }
				],
				dynamicTitle: true
			}
		}
	},
	initialGlobals: { skin: 'os', motion: 'lively' },
	decorators: [
		(_story, context) => ({
			Component: StoryFrame,
			props: {
				skin: parseSkinChoice(context.globals.skin),
				motion: parseMotion(context.globals.motion),
				layer: context.tags.includes('layer')
			}
		})
	],
	parameters: {
		a11y: {
			test: 'error',
			options: {
				runOnly: {
					type: 'tag',
					values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']
				}
			}
		},
		// The skin owns the page's colour.
		backgrounds: { disable: true },
		options: {
			storySort: {
				order: [
					'Tokens and Paper',
					'Controls',
					'Surfaces and Feedback',
					'Navigation and Patterns',
					'Motion and Sound',
					'Heavy Pieces',
					"Pieces for 0.3's Screens"
				]
			}
		}
	},
	// Under Still nothing moves: a running animation fails the story.
	afterEach: ({ globals }) => {
		if (parseMotion(globals.motion) !== 'still') return;
		const running = document
			.getAnimations()
			.filter((animation) => animation.playState === 'running');
		expect(running, 'a running animation under Still').toEqual([]);
	}
};

export default preview;
