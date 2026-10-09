<script lang="ts">
	/**
	 * An on/off switch with its label beside it. `onchange` gets the new
	 * value; the caller decides what flipping means.
	 */
	let {
		checked,
		label,
		onchange,
		testid
	}: {
		checked: boolean;
		label: string;
		onchange: (next: boolean) => void;
		testid?: string | undefined;
	} = $props();
</script>

<button
	type="button"
	class="toggle"
	role="switch"
	aria-checked={checked}
	data-testid={testid}
	onclick={() => {
		onchange(!checked);
	}}
>
	<span class="toggle__track" aria-hidden="true"><span class="toggle__thumb"></span></span>
	<span class="toggle__label">{label}</span>
</button>

<style>
	.toggle {
		display: inline-flex;
		align-items: flex-start;
		gap: var(--space-2);
		padding: var(--space-1) 0;
		border: 0;
		background: transparent;
		color: var(--color-text);
		font-size: var(--text-sm);
		text-align: left;
	}

	.toggle__track {
		position: relative;
		flex: none;
		width: var(--space-8);
		height: var(--space-5);
		margin-top: calc(var(--space-1) / 2);
		border: var(--border-width) solid var(--color-border);
		border-radius: calc(var(--space-5) / 2);
		background: var(--color-surface-raised);
		transition: background-color var(--duration-fast) ease;
	}

	.toggle__thumb {
		position: absolute;
		top: calc((var(--space-5) - var(--space-3)) / 2 - var(--border-width));
		left: calc((var(--space-5) - var(--space-3)) / 2 - var(--border-width));
		width: var(--space-3);
		height: var(--space-3);
		border-radius: var(--radius-round);
		background: var(--color-text-muted);
		transition: transform var(--duration-fast) ease;
	}

	.toggle[aria-checked='true'] .toggle__track {
		background: var(--color-accent);
		border-color: var(--color-accent);
	}

	.toggle[aria-checked='true'] .toggle__thumb {
		background: var(--color-on-accent);
		transform: translateX(calc(var(--space-8) - var(--space-5)));
	}

	@media (forced-colors: active) {
		.toggle__track {
			border: 1px solid CanvasText;
		}

		.toggle[aria-checked='true'] .toggle__thumb {
			background: Highlight;
		}
	}
</style>
