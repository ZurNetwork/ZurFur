<script lang="ts">
	import { useEnvironment, type Environment } from '../../lib/environment/context.ts';
	import EnvironmentProbe from './EnvironmentProbe.svelte';

	interface Props {
		/** How many probes deeper the one that reads the environment sits; 0 reads it here. */
		readonly depth: number;
		/** Handed the environment the deepest probe found. */
		readonly onEnvironment: (environment: Environment) => void;
	}

	let { depth, onEnvironment }: Props = $props();

	/** Reads the environment here when this is the deepest probe; called once, at init. */
	function report(): void {
		if (depth === 0) onEnvironment(useEnvironment());
	}

	report();
</script>

{#if depth > 0}
	<EnvironmentProbe depth={depth - 1} {onEnvironment} />
{/if}
