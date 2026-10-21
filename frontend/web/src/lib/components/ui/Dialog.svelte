<script lang="ts">
	import type { Snippet } from 'svelte';
	import IconButton from './IconButton.svelte';

	/**
	 * A centred modal; the drawer, a side sheet from the left; or a centred
	 * modal that fills the screen at the phone breakpoint and narrower.
	 */
	type DialogVariant = 'modal' | 'drawer' | 'full-screen-on-phone';

	/**
	 * A modal dialog over an inert page, named by its visible title. Opening it
	 * focuses the element whose id is `initialFocus`, else its close button;
	 * Escape, the close button or a click on the backdrop closes it, and focus
	 * goes to `returnFocus`, else to whatever had focus when it opened. Its
	 * body and footer exist only while open; the footer stays put while the
	 * body scrolls.
	 */
	let {
		open = $bindable(false),
		label,
		variant = 'modal',
		id,
		initialFocus,
		returnFocus,
		children,
		footer
	}: {
		open?: boolean;
		label: string;
		variant?: DialogVariant;
		id?: string | undefined;
		initialFocus?: string | undefined;
		returnFocus?: HTMLElement | undefined;
		children: Snippet;
		footer?: Snippet | undefined;
	} = $props();

	const ownId = $props.id();
	const titleId = `${ownId}-title`;

	let dialog: HTMLDialogElement | undefined = $state();
	let closeButton: HTMLButtonElement | undefined = $state();
	let opener: HTMLElement | undefined;

	$effect(() => {
		if (dialog === undefined) return;
		if (open && !dialog.open) {
			const active = document.activeElement;
			opener = active instanceof HTMLElement ? active : undefined;
			dialog.showModal();
			focusOnOpen(dialog);
		} else if (!open && dialog.open) {
			dialog.close();
		}
	});

	/** The chosen element inside the dialog, if it is there; else the close button. */
	function focusOnOpen(element: HTMLDialogElement): void {
		const chosen =
			initialFocus === undefined
				? undefined
				: element.querySelector(`#${CSS.escape(initialFocus)}`);
		if (chosen instanceof HTMLElement) {
			chosen.focus();
			return;
		}
		closeButton?.focus();
	}

	/** The dialog closed, by any route: sync the flag and give focus back. */
	function closed(): void {
		open = false;
		(returnFocus ?? opener)?.focus();
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
	aria-labelledby={titleId}
	{id}
	onclose={closed}
	onclick={backdropClick}
>
	<div class="dialog__sheet">
		<div class="dialog__header">
			<span class="dialog__title" id={titleId}>{label}</span>
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
			{#if footer !== undefined}
				<div class="dialog__footer">{@render footer()}</div>
			{/if}
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

	/* Open, the dialog is a column the sheet fills, so only the body scrolls. */
	.dialog[open] {
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.dialog::backdrop {
		background: var(--color-backdrop);
	}

	.dialog--modal {
		border-radius: var(--radius-lg);
		max-width: min(var(--dialog-width), calc(100vw - 2 * var(--space-4)));
	}

	/* A centred modal that fills a phone's screen: base.css switches these at its breakpoint. */
	.dialog--full-screen-on-phone {
		width: var(--full-dialog-width);
		max-width: var(--full-dialog-max-width);
		height: var(--full-dialog-height);
		max-height: var(--full-dialog-max-height);
		margin: var(--full-dialog-margin);
		border-width: var(--full-dialog-border-width);
		border-radius: var(--full-dialog-radius);
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
		flex: 1;
		flex-direction: column;
		min-height: 0;
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

	.dialog__footer {
		display: flex;
		flex: none;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4);
		border-top: var(--border-width) solid var(--color-border);
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
