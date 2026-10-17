<script lang="ts">
	import { superForm, type SuperValidated } from 'sveltekit-superforms';
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import type { CommissionId, DenName } from '$lib/types/brand';
	import { denRootFallback } from './den-root';
	import { treeMemoryFromContext } from './tree-memory.svelte';

	/**
	 * "Rename": a button that opens a dialog with the Title field, starting
	 * from the commission's `name`, and a hidden field carrying its `id`. A
	 * refused title shows under its field, any other problem below the form.
	 * Closing the dialog clears the form; a redirect also starts the tree over.
	 */
	let {
		form: given,
		id,
		name
	}: {
		form: SuperValidated<{ id: string; title: string }>;
		id: CommissionId;
		name: DenName;
	} = $props();

	let open = $state(false);
	/** Whether the dialog was open when the last close was looked for. */
	let wasOpen = false;
	const treeMemory = treeMemoryFromContext();
	// superForm seeds from the initial value by design and manages its own
	// reactivity from there.
	// svelte-ignore state_referenced_locally
	const { form, errors, enhance, message, reset } = superForm(given, {
		onError: 'apply',
		onResult: ({ result }) => {
			if (result.type !== 'redirect') return;
			open = false;
			treeMemory?.restart(denRootFallback());
		}
	});

	$effect(() => {
		if (wasOpen && !open) reset();
		wasOpen = open;
	});
</script>

<div class="rename-commission">
	<Button
		onclick={() => {
			$form.title = name;
			open = true;
		}}
		testid="rename-commission">Rename</Button
	>
</div>

<Dialog bind:open label="Rename">
	<form class="rename-commission__form" method="post" action="?/renameCommission" use:enhance>
		<input type="hidden" name="id" value={id} />
		<label>
			Title
			<input
				name="title"
				bind:value={$form.title}
				aria-invalid={$errors.title ? 'true' : undefined}
				aria-describedby={$errors.title ? 'rename-commission-title-error' : undefined}
			/>
		</label>
		{#if $errors.title}
			<ul
				class="rename-commission__errors"
				role="alert"
				id="rename-commission-title-error"
				data-testid="rename-title-error"
			>
				{#each $errors.title as errorMessage (errorMessage)}
					<li>{errorMessage}</li>
				{/each}
			</ul>
		{/if}
		{#if $message}
			<ProblemNote problem={$message} />
		{/if}
		<div class="rename-commission__actions">
			<Button variant="primary" type="submit">Save</Button>
			<Button
				onclick={() => {
					open = false;
				}}>Cancel</Button
			>
		</div>
	</form>
</Dialog>

<style>
	.rename-commission {
		margin-bottom: var(--space-4);
	}

	.rename-commission__form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-4);
	}

	.rename-commission__errors {
		margin: 0;
		padding-left: var(--space-4);
		color: var(--color-danger);
		font-size: var(--text-sm);
	}

	.rename-commission__actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
