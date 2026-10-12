<script lang="ts">
	import type { Snippet } from 'svelte';
	import IconButton from './IconButton.svelte';

	/** A centred modal, or the drawer: a side sheet from the left. */
	type DialogVariant = 'modal' | 'drawer';

	/**
	 * A modal dialog over an inert page. Opening it focuses its close button;
	 * Escape, the close button or a click on the backdrop closes it, and focus
	 * goes back to whatever opened it. Its content exists only while open.
	 */
	let {
		open = $bindable(false),
		label,
		variant = 'modal',
		id,
		children
	}: {
		open?: boolean;
		label: string;
		variant?: DialogVariant;
		id?: string | undefined;
		children: Snippet;
	} = $props();

	let dialog: HTMLDialogElement | undefined = $state();
	let closeButton: HTMLButtonElement | undefined = $state();
	let opener: HTMLElement | undefined;

	$effect(() => {
		if (dialog === undefined) return;
		if (open && !dialog.open) {
			const active = document.activeElement;
			opener = active instanceof HTMLElement ? active : undefined;
			dialog.showModal();
			closeButton?.focus();
		} else if (!open && dialog.open) {
			dialog.close();
		}
	});

	/** The dialog closed, by any route: sync the flag and give focus back. */
	function closed(): void {
		open = false;
		opener?.focus();
		opener = undefined;
	}

	/** A click on the backdrop lands on the dialog element itself. */
	function backdropClick(event: MouseEvent): void {
		if (event.target === dialog) open = false;
	}
</script>

<!-- The backdrop click is a mouse shortcut; Escape and the close button are the keyboard routes. -->
<dialog
	bind:this={dialog}
	class="dialog dialog--{variant}"
	aria-label={label}
	{id}
	onclose={closed}
	onclick={backdropClick}
>
	<div class="dialog__sheet">
		<div class="dialog__header">
			<span class="dialog__title">{label}</span>
			<IconButton
				icon="x"
				label="Close"
				bind:element={closeButton}
				onclick={() => (open = false)}
				testid="dialog-close"
			/>
		</div>
		{#if open}
			<div class="dialog__body">{@render children()}</div>
		{/if}
	</div>
</dialog>

<style>
	.dialog {
		padding: 0;
		border: var(--border-width) solid var(--color-border);
		background: var(--color-surface);
		color: var(--color-text);
		box-shadow: var(--elevation-2);
	}

	.dialog::backdrop {
		background: var(--color-backdrop);
	}

	.dialog--modal {
		border-radius: var(--radius-lg);
		max-width: min(var(--dialog-width), calc(100vw - 2 * var(--space-4)));
	}

	.dialog--drawer {
		margin: 0;
		width: var(--drawer-width);
		max-width: none;
		height: 100%;
		max-height: none;
		border-width: 0 var(--border-width) 0 0;
	}

	.dialog--drawer[open] {
		animation: drawer-in var(--duration-normal) ease-out;
	}

	.dialog__sheet {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.dialog__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: var(--topbar-height);
		padding: 0 var(--space-2) 0 var(--space-4);
		border-bottom: var(--border-width) solid var(--color-border);
	}

	.dialog__title {
		font-weight: var(--text-weight-bold);
	}

	.dialog__body {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}

	@keyframes drawer-in {
		from {
			transform: translateX(-100%);
		}

		to {
			transform: translateX(0);
		}
	}
</style>
