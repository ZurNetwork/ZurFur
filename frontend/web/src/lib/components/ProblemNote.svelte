<script lang="ts">
	import type { Problem } from '$lib/api/problem';

	/**
	 * The one way a screen renders an RFC 9457 problem: the detail —
	 * REQUIRED by the contract's one Problem declaration — with
	 * the title as a defense-in-depth fallback should an empty detail ever
	 * slip past the backend's registry assert. Every later screen reuses this
	 * instead of re-inventing per-page error markup.
	 */
	let { problem }: { problem: Problem } = $props();
</script>

<p role="alert" data-testid="problem" data-problem-code={problem.code}>
	{problem.detail || problem.title}
</p>
