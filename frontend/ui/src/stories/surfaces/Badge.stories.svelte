<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { Badge, ICON_NAMES } from '@zurfur/ui';
	import { badgeTones, badgeWords } from '../fixtures/surfaces.ts';

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Badge',
		component: Badge,
		tags: ['autodocs', 'simple'],
		args: { label: badgeWords.neutral },
		argTypes: {
			tone: {
				control: 'select',
				options: ['neutral', 'accent', 'iris', 'crown', 'rose', 'taupe']
			},
			icon: { control: 'select', options: [...ICON_NAMES] },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a small tag for a short fact, such as a state, a kind or a count of something. It takes six tones: Neutral is the default, Accent marks what is chosen, and Iris, Crown, Rose and Taupe tell kinds apart. The label names the kind, and the tone only repeats it. The label is plain text and gets cut short when it is too long. An icon can sit before it.`
				}
			}
		}
	});
</script>

{#snippet tones(_args: unknown)}
	{#each badgeTones as tone (tone)}
		<Badge {tone} label={badgeWords[tone]} />
	{/each}
{/snippet}

{#snippet withIcon(_args: unknown)}
	<Badge icon="clock" label={badgeWords.neutral} />
	<Badge icon="circle-check" tone="accent" label={badgeWords.accent} />
{/snippet}

<Story name="Neutral" />

<Story name="Tones" template={tones} />

<Story name="With an Icon" template={withIcon} />
