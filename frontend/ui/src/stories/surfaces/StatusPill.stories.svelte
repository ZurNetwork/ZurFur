<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { StatusPill } from '@zurfur/ui';
	import { lateFact, onePart, twoParts } from '../fixtures/surfaces.ts';

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Status Pill',
		component: StatusPill,
		tags: ['autodocs', 'simple'],
		args: { parts: onePart },
		argTypes: {
			parts: { control: 'object' },
			fact: { table: { disable: true } },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: where a comm stands, in one pill or two. Each part has a label, a tone and maybe an icon. The tones are Neutral, Progress, Waiting and Done, and there is no danger tone. Two parts sit side by side with a thin edge between them. A quiet fact can sit beside the pill, such as a date that has passed.`
				}
			}
		}
	});
</script>

<Story name="One Part" />

<Story name="Two Parts" args={{ parts: twoParts }} />

<Story name="With a Fact" args={{ parts: onePart, fact: lateFact }} />
