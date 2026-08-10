<script lang="ts">
	import type { PageData } from './$types';
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import { superForm } from 'sveltekit-superforms';

	let { data }: { data: PageData } = $props();
	// superForm seeds from the initial load value by design and manages its
	// own reactivity from there.
	// svelte-ignore state_referenced_locally
	const { form, errors, enhance, constraints, message } = superForm(data.form);
</script>

<svelte:head>
	<title>Sign in — Zurfur</title>
</svelte:head>

<h1>Sign in</h1>

<!-- Deliberately NOT ProblemNote: callback errors are redirect codes with local
     copy (callback-errors.ts), not RFC 9457 problems off the wire — minting a
     fake Problem for them would misuse the seam. -->
{#if data.callbackError !== undefined}
	<p role="alert" data-testid="callback-error">{data.callbackError}</p>
{/if}

<form method="post" use:enhance>
	<label>
		Handle
		<input
			name="handle"
			placeholder="you.bsky.social"
			autocomplete="username"
			bind:value={$form.handle}
			aria-invalid={$errors.handle ? 'true' : undefined}
			aria-describedby={$errors.handle ? 'handle-error' : undefined}
			{...$constraints.handle}
		/>
	</label>
	{#if $errors.handle}
		<ul role="alert" id="handle-error" data-testid="handle-error">
			{#each $errors.handle as errorMessage (errorMessage)}
				<li>{errorMessage}</li>
			{/each}
		</ul>
	{/if}
	<button>Sign in</button>
</form>

{#if $message}
	<ProblemNote problem={$message} />
{/if}
