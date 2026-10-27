<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect } from 'storybook/test';
	import { Icon, ICON_NAMES, type IconName } from '@zurfur/ui';

	const { Story } = defineMeta({
		title: 'Controls/Icon',
		component: Icon,
		tags: ['autodocs'],
		args: { name: 'send' },
		argTypes: {
			name: { control: 'select', options: [...ICON_NAMES] },
			size: { control: 'inline-radio', options: ['sm', 'md', 'lg'] },
			weight: { control: 'inline-radio', options: ['regular', 'heavy'] }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: one small drawing from the icon set. It takes its colour from the text around it, and it is hidden from screen readers, so put the words next to it or on the button that holds it. Small is for pills, badges and twisties, Medium is the everyday size, Large is for the rail, empty states and toasts. Heavy is for the tick inside a checked box.`
				}
			}
		}
	});

	/** How many icons one row of the full set holds. */
	const perRow = 6;

	/** Every icon name, cut into rows. */
	const rows: readonly (readonly IconName[])[] = Array.from(
		{ length: Math.ceil(ICON_NAMES.length / perRow) },
		(_unused, row) => ICON_NAMES.slice(row * perRow, (row + 1) * perRow)
	);
</script>

{#snippet sizes(_args: unknown)}
	<table>
		<tbody>
			<tr>
				<th scope="row">Small</th>
				<td><Icon name="send" size="sm" /></td>
			</tr>
			<tr>
				<th scope="row">Medium</th>
				<td><Icon name="send" size="md" /></td>
			</tr>
			<tr>
				<th scope="row">Large</th>
				<td><Icon name="send" size="lg" /></td>
			</tr>
		</tbody>
	</table>
{/snippet}

{#snippet weights(_args: unknown)}
	<table>
		<tbody>
			<tr>
				<th scope="row">Regular</th>
				<td><Icon name="check" size="lg" /></td>
			</tr>
			<tr>
				<th scope="row">Heavy</th>
				<td><Icon name="check" size="lg" weight="heavy" /></td>
			</tr>
		</tbody>
	</table>
{/snippet}

{#snippet everyIcon(_args: unknown)}
	<table>
		<tbody>
			{#each rows as row, index (index)}
				<tr>
					{#each row as name (name)}
						<td><Icon {name} /><code>{name}</code></td>
					{/each}
				</tr>
			{/each}
		</tbody>
	</table>
{/snippet}

<Story
	name="Sizes"
	template={sizes}
	play={async ({ canvasElement }) => {
		const widths = [...canvasElement.querySelectorAll('svg')].map(
			(drawing) => getComputedStyle(drawing).width
		);
		await expect(widths).toEqual(['14px', '16px', '20px']);
	}}
/>

<Story name="Weights" template={weights} />

<Story
	name="Every Icon"
	template={everyIcon}
	play={async ({ canvasElement }) => {
		await expect(canvasElement.querySelectorAll('svg')).toHaveLength(ICON_NAMES.length);
	}}
/>

<Story name="Playground" args={{ size: 'lg' }} />
