<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { fn } from 'storybook/test';
	import { Button, Card, Skeleton } from '@zurfur/ui';
	import type { CardElement, CardPadding } from '@zurfur/ui';
	import { cardList, cardWords } from '../fixtures/surfaces.ts';

	/** What a template here reads from the story's args. */
	interface Shown {
		readonly as?: CardElement | undefined;
		readonly padding?: CardPadding | undefined;
	}

	/** Stands in for the press the Button inside the card reports. */
	const onPress = fn();

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Card',
		component: Card,
		render: template,
		tags: ['autodocs', 'simple'],
		argTypes: {
			as: { control: 'inline-radio', options: ['div', 'section', 'article', 'li'] },
			padding: { control: 'inline-radio', options: ['default', 'none'] }
		},
		parameters: {
			docs: {
				description: {
					component: `A simple component: a sheet laid on the page, for one thing you act on, such as a comm, a ref or a piece. It has no title or action slots of its own, so it holds whatever it is given. Only objects are cards. A group of things is never one card. It is a div unless you ask for a section, an article or a list item; any other element becomes a div. No padding is for a card that holds a picture edge to edge.`
				}
			}
		}
	});
</script>

{#snippet template(args: Shown)}
	<Card as={args.as} padding={args.padding}>
		<p>{cardWords.title}</p>
		<p>{cardWords.note}</p>
	</Card>
{/snippet}

{#snippet flush(_args: Shown)}
	<Card padding="none">
		<Skeleton shape="rect" />
	</Card>
{/snippet}

{#snippet article(_args: Shown)}
	<Card as="article">
		<p>{cardWords.article}</p>
		<p>{cardWords.articleNote}</p>
	</Card>
{/snippet}

{#snippet inAList(_args: Shown)}
	<ul>
		{#each cardList as work (work)}
			<Card as="li">
				<p>{work}</p>
			</Card>
		{/each}
	</ul>
{/snippet}

{#snippet withActions(_args: Shown)}
	<Card>
		<p>{cardWords.title}</p>
		<Button variant="secondary" {onPress}>{cardWords.action}</Button>
	</Card>
{/snippet}

<Story name="Default" />

<Story name="No Padding" template={flush} />

<Story name="As an Article" template={article} />

<Story name="In a List" template={inAList} />

<Story name="With Actions" template={withActions} />
