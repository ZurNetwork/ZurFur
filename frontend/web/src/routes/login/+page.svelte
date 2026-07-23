<script lang="ts">
	import type { ActionData, PageData } from './$types';
	import ProblemNote from '$lib/components/ProblemNote.svelte';

	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head>
	<title>Sign in — Zurfur</title>
</svelte:head>

<h1>Sign in</h1>

<!-- Deliberately NOT ProblemNote: callback errors are redirect codes with local
     copy (callback-errors.ts), not RFC 9457 problems off the wire — minting a
     fake Problem for them would misuse the seam. -->
{#if data.callbackError !== null}
	<p role="alert" data-testid="callback-error">{data.callbackError}</p>
{/if}

<form method="post">
	<label>
		Handle
		<input name="handle" placeholder="you.bsky.social" autocomplete="username" />
	</label>
	<button>Sign in</button>
</form>

{#if form?.problem}
	<ProblemNote problem={form.problem} />
{/if}
