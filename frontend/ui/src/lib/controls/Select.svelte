<script lang="ts" generics="V extends string">
	import { tick } from 'svelte';
	import Icon from '../icons/Icon.svelte';
	import FieldFrame from './FieldFrame.svelte';
	import type { FieldIds, SelectOption } from './types';

	interface Props {
		readonly label: string;
		readonly options: readonly SelectOption<V>[];
		/** The chosen option's value, or `undefined` while nothing is chosen. */
		readonly value: V | undefined;
		/** Handed the option's value; the parent decides whether `value` follows. */
		readonly onChange: (value: V) => void;
		/** A disabled first option, shown while nothing is chosen. */
		readonly placeholder?: string | undefined;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		readonly disabled?: boolean;
		readonly required?: boolean;
		/** With a name, a form can post the choice; without one it never does. */
		readonly name?: string | undefined;
		readonly testid?: string | undefined;
	}

	let {
		label,
		options,
		value,
		onChange,
		placeholder,
		hint,
		error,
		disabled = false,
		required = false,
		name,
		testid
	}: Props = $props();

	/** Where the shown option sits in the list, the placeholder counted; -1 when nothing fits. */
	const shownIndex = $derived.by(() => {
		const found = options.findIndex((option) => option.value === value);
		if (placeholder === undefined) return found;
		return found === -1 ? 0 : found + 1;
	});

	/** Shows the option `value` names, again whenever `value` or the options change. */
	function show(select: HTMLSelectElement): void {
		select.selectedIndex = shownIndex;
	}

	const classes = $derived([
		'zf-select',
		'slip',
		error !== undefined && 'zf-select--invalid',
		disabled && 'zf-select--disabled'
	]);

	/**
	 * A choice: looks the DOM's text up in the options and hands the parent that
	 * option's value, then shows whatever `value` says next.
	 */
	async function choose(
		event: Event & { readonly currentTarget: HTMLSelectElement }
	): Promise<void> {
		const select = event.currentTarget;
		const chosen = options.find((option) => option.value === select.value);
		if (chosen !== undefined) onChange(chosen.value);
		await tick();
		select.selectedIndex = shownIndex;
	}
</script>

<FieldFrame {label} {hint} {error}>
	{#snippet children(ids: FieldIds)}
		<span class={classes}>
			<select
				onchange={choose}
				aria-labelledby={ids.labelId}
				aria-describedby={ids.describedBy}
				aria-invalid={ids.invalid}
				data-testid={testid}
				{name}
				{disabled}
				{required}
				{@attach show}
			>
				{#if placeholder !== undefined}
					<option value="" disabled>{placeholder}</option>
				{/if}
				{#each options as option (option.value)}
					<option value={option.value}>{option.label}</option>
				{/each}
			</select>
			<span class="zf-select__chevron"><Icon name="chevron-down" /></span>
		</span>
	{/snippet}
</FieldFrame>
