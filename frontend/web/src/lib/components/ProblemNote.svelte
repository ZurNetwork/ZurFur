<script lang="ts">
	import type { Problem } from '$lib/api/problem';

	/**
	 * The one way a screen renders an RFC 9457 problem (ZMVP-151): detail when
	 * the backend sent one, title otherwise. Every later screen reuses this
	 * instead of re-inventing per-page error markup.
	 */
	let { problem }: { problem: Problem } = $props();
</script>

<p role="alert" data-testid="problem" data-problem-code={problem.code}>
	{problem.detail ?? problem.title}
</p>
