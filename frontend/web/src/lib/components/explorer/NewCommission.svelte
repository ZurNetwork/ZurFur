<script lang="ts">
	import { superForm, type SuperValidated } from 'sveltekit-superforms';
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import { denRootFallback } from './den-root';
	import { treeMemoryFromContext } from './tree-memory.svelte';

	/**
	 * "New commission": a button that opens a dialog with the Title field and
	 * the line saying new commissions are private, with no choice of level.
	 * A refused title shows under its field, any other problem below the
	 * form. A redirect closes the dialog and starts the tree over.
	 */
	let { form: given }: { form: SuperValidated<{ title: string }> } = $props();

	let open = $state(false);
	const treeMemory = treeMemoryFromContext();
	// superForm seeds from the initial value by design and manages its own
	// reactivity from there.
	// svelte-ignore state_referenced_locally
	const { form, errors, enhance, message, reset } = superForm(given, {
		onError: 'apply',
		onResult: ({ result }) => {
			if (result.type !== 'redirect') return;
			open = false;
			reset();
			treeMemory?.restart(denRootFallback());
		}
	});
</script>

<div class="new-commission">
	<Button
		variant="primary"
		onclick={() => {
			open = true;
		}}
		testid="new-commission">New commission</Button
	>
</div>

<Dialog bind:open label="New commission">
	<form class="new-commission__form" method="post" action="?/createCommission" use:enhance>
		<label>
			Title
			<input
				name="title"
				bind:value={$form.title}
				aria-invalid={$errors.title ? 'true' : undefined}
				aria-describedby={$errors.title ? 'new-commission-title-error' : undefined}
			/>
		</label>
		{#if $errors.title}
			<ul
				class="new-commission__errors"
				role="alert"
				id="new-commission-title-error"
				data-testid="title-error"
			>
				{#each $errors.title as errorMessage (errorMessage)}
					<li>{errorMessage}</li>
				{/each}
			</ul>
		{/if}
		<p class="new-commission__private" data-testid="new-commission-private">
			New commissions are private.
		</p>
		{#if $message}
			<ProblemNote problem={$message} />
		{/if}
		<div class="new-commission__actions">
			<Button variant="primary" type="submit">Create</Button>
			<Button
				onclick={() => {
					open = false;
				}}>Cancel</Button
			>
		</div>
	</form>
</Dialog>

<style>
	.new-commission {
		margin-bottom: var(--space-4);
	}

	.new-commission__form {
		display: flex;
		flex-direction: column;
		padding: var(--space-4);
	}

	.new-commission__errors {
		margin: 0 0 var(--space-3);
		padding-left: var(--space-4);
		color: var(--color-danger);
		font-size: var(--text-sm);
	}

	.new-commission__private {
		margin: 0 0 var(--space-4);
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	.new-commission__actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
