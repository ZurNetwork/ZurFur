<script lang="ts" generics="Id extends string">
	import { RadioGroup } from 'bits-ui';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import type { PillOption } from './types';

	interface Props {
		/** The group's accessible name. */
		readonly label: string;
		readonly options: readonly PillOption<Id>[];
		/** The id of the option chosen; the parent decides whether it follows `onChange`. */
		readonly value: Id;
		/** Handed the id of the option an interaction asks for. */
		readonly onChange: (id: Id) => void;
		readonly disabled?: boolean;
		/** With a name, a hidden input carries the value in a form. */
		readonly name?: string | undefined;
		/** Put on the group. */
		readonly testid?: string | undefined;
	}

	let { label, options, value, onChange, disabled = false, name, testid }: Props = $props();

	const { moves, sounds } = useEnvironment();

	/** A choice asked for: unknown ids, the empty string and disabled options are ignored; the rest play the cue and ask the parent. */
	function choose(next: string): void {
		const asked = options.find((option) => option.id === next);
		if (next === '' || asked === undefined || asked.disabled === true) return;
		sounds.play('toggle');
		onChange(asked.id);
	}
</script>

<!-- The group, shared by both roots below: bits-ui's props on the div, its options inside. -->
{#snippet child({ props }: { readonly props: Record<string, unknown> })}
	<div {...props} class="zf-seg piece" data-testid={testid}>
		{#each options as option (option.id)}
			<RadioGroup.Item value={option.id} disabled={option.disabled ?? false}>
				{#snippet child({ props })}
					<button
						{...props}
						class={['zf-seg__opt', 'pick', option.prefix !== undefined && 'zf-seg__opt--typed']}
						{@attach moves.squish()}
					>
						{#if option.icon !== undefined}<Icon name={option.icon} size="sm" />{/if}
						{#if option.prefix !== undefined}<span class="zf-seg__prefix">{option.prefix}</span
							>{/if}
						<span>{option.label}</span>
					</button>
				{/snippet}
			</RadioGroup.Item>
		{/each}
	</div>
{/snippet}

<!-- bits-ui's `name` takes no undefined, so the root without a name is its own branch. -->
{#if name === undefined}
	<RadioGroup.Root
		aria-label={label}
		orientation="horizontal"
		bind:value={() => value, choose}
		{disabled}
		{child}
	/>
{:else}
	<RadioGroup.Root
		aria-label={label}
		orientation="horizontal"
		bind:value={() => value, choose}
		{disabled}
		{name}
		{child}
	/>
{/if}
