<script lang="ts">
	import { resolve } from '$app/paths';
	import Button from '$lib/components/ui/Button.svelte';

	/**
	 * Sign out: a plain, unenhanced form post, so the whole page reloads and
	 * nothing the frame held in memory survives into the next session.
	 */
</script>

<form method="post" action={resolve('/logout')}>
	<Button variant="quiet" type="submit">Sign out</Button>
</form>
