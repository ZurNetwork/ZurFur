<script lang="ts">
	import type { ResolvedPathname } from '$app/types';
	import type { Snippet } from 'svelte';

	/** The four looks a Button takes. */
	type ButtonVariant = 'primary' | 'secondary' | 'quiet' | 'destructive';

	/** What every Button has. */
	interface ButtonBase {
		variant?: ButtonVariant;
		testid?: string | undefined;
		children: Snippet;
	}

	/** A link that looks like a button: navigation only. */
	interface LinkButton extends ButtonBase {
		href: ResolvedPathname;
	}

	/** A real button: an action, which can be busy. */
	interface ActionButton extends ButtonBase {
		href?: never;
		type?: 'button' | 'submit';
		busy?: boolean;
		onclick?: ((event: MouseEvent) => void) | undefined;
	}

	/**
	 * A labelled action. With `href` it is a link that looks like a button
	 * (and takes nothing a link would ignore); otherwise a real `<button>`.
	 * `busy` keeps it in place, marks it busy and stops further presses.
	 */
	// eslint-disable-next-line svelte/no-unused-props -- every prop is read through the union, which destructuring would lose
	let props: LinkButton | ActionButton = $props();

	const variant = $derived(props.variant ?? 'secondary');

	function press(event: MouseEvent, action: ActionButton): void {
		if (action.busy === true) {
			event.preventDefault();
			return;
		}
		action.onclick?.(event);
	}
</script>

{#if props.href !== undefined}
	<a class="button button--{variant}" href={props.href} data-testid={props.testid}
		>{@render props.children()}</a
	>
{:else}
	{@const action = props}
	<button
		class="button button--{variant}"
		type={action.type ?? 'button'}
		aria-busy={action.busy === true ? 'true' : undefined}
		aria-disabled={action.busy === true ? 'true' : undefined}
		onclick={(event) => {
			press(event, action);
		}}
		data-testid={action.testid}>{@render action.children()}</button
	>
{/if}

<style>
	.button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		min-height: var(--space-9);
		padding: var(--space-1) var(--space-4);
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-md);
		font-size: var(--text-sm);
		font-weight: var(--text-weight-medium);
		text-decoration: none;
		line-height: 1.2;
		white-space: nowrap;
		transition: background-color var(--duration-fast) ease;
	}

	.button--primary {
		background: var(--color-accent);
		color: var(--color-on-accent);
	}

	.button--secondary {
		background: var(--color-surface-raised);
		border-color: var(--color-border);
		color: var(--color-text);
	}

	.button--secondary:hover,
	.button--quiet:hover {
		background: var(--color-hover);
	}

	.button--quiet {
		background: transparent;
		color: var(--color-text);
	}

	.button--destructive {
		background: transparent;
		border-color: var(--color-danger);
		color: var(--color-danger);
	}

	.button[aria-busy='true'] {
		cursor: progress;
		opacity: 0.7;
	}
</style>
