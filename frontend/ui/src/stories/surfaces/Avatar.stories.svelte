<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect } from 'storybook/test';
	import { Avatar } from '@zurfur/ui';
	import {
		avatarPicture,
		avatarSizes,
		avatarWords,
		withoutIsolates
	} from '../fixtures/surfaces.ts';

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Avatar',
		component: Avatar,
		tags: ['autodocs', 'simple'],
		args: { name: avatarWords.name, size: 'md', tone: 'neutral', textShown: true },
		argTypes: {
			size: { control: 'inline-radio', options: ['sm', 'md', 'lg'] },
			tone: {
				control: 'select',
				options: ['neutral', 'accent', 'iris', 'crown', 'rose', 'taupe']
			},
			textShown: { control: 'boolean' },
			picture: { table: { disable: true } },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: someone's picture in a circle, with their name and handle beside it, or alone. Without a picture, or when it fails to load, the circle shows the first letter, number or emoji of the name, in one of six tones. Only a picture from this site loads. Alone, the circle is read out as the name. The name is always plain text, never markup.`
				}
			}
		}
	});
</script>

{#snippet sizes(_args: unknown)}
	{#each avatarSizes as size (size)}
		<Avatar name={avatarWords.name} {size} tone="iris" textShown={false} />
	{/each}
{/snippet}

<Story name="Picture" args={{ picture: avatarPicture, handle: avatarWords.handle }} />

<Story name="Initial" args={{ handle: avatarWords.handle, tone: 'crown' }} />

<Story name="Marks Clipped" args={{ name: avatarWords.marks, tone: 'rose' }} />

<Story name="Emoji Name" args={{ name: avatarWords.emoji, tone: 'accent' }} />

<Story name="No Letters" args={{ name: avatarWords.noLetters, tone: 'taupe' }} />

<Story
	name="Alone"
	args={{ textShown: false, picture: avatarPicture }}
	play={async ({ canvas }) => {
		const circle = canvas.getByRole('img', {
			name: (heard) => withoutIsolates(heard) === avatarWords.name
		});
		await expect(circle).toBeVisible();
	}}
/>

<Story name="Sizes" template={sizes} />

<Story
	name="Hostile Name"
	args={{ name: avatarWords.hostile }}
	play={async ({ canvasElement, canvas }) => {
		await expect(canvasElement.querySelector('img')).toBeNull();
		await expect(canvas.getByText(avatarWords.hostile)).toBeVisible();
	}}
/>
