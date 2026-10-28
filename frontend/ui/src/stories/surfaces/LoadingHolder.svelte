<script lang="ts">
	import { Button, Skeleton, Spinner } from '@zurfur/ui';

	interface Props {
		readonly action: string;
		readonly label: string;
		readonly onStart: () => void;
		readonly lines?: number;
	}

	let { action, label, onStart, lines = 3 }: Props = $props();

	/** Whether loading has started: the status stays mounted and is turned on, so its words are announced. */
	let loading = $state(false);

	/** Hands the press to the story's callback, then starts loading. */
	function start(): void {
		onStart();
		loading = true;
	}
</script>

<Button onPress={start} testid="start">{action}</Button>
<Spinner kind="status" {label} active={loading} testid="status" />
{#if loading}
	<Skeleton shape="lines" {lines} testid="blank" />
{/if}
