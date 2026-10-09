<script lang="ts">
	import Icon from './Icon.svelte';
	import type { IconName } from './icons';

	/**
	 * A button that shows only an icon; `label` is its accessible name.
	 * `element` binds the underlying button, so a caller can move focus to it.
	 */
	let {
		icon,
		label,
		onclick,
		expanded,
		controls,
		element = $bindable(),
		testid
	}: {
		icon: IconName;
		label: string;
		onclick: (event: MouseEvent) => void;
		expanded?: boolean | undefined;
		controls?: string | undefined;
		element?: HTMLButtonElement | undefined;
		testid?: string | undefined;
	} = $props();
</script>

<button
	bind:this={element}
	class="icon-button"
	type="button"
	aria-label={label}
	aria-expanded={expanded}
	aria-controls={controls}
	data-testid={testid}
	{onclick}
>
	<Icon name={icon} size="1.25rem" />
</button>

<style>
	.icon-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--space-10);
		height: var(--space-10);
		padding: 0;
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--color-text);
	}

	.icon-button:hover {
		background: var(--color-hover);
	}
</style>
