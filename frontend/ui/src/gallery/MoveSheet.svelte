<script lang="ts">
	interface Props {
		/** A class from the paper moves. */
		readonly move: string;
	}

	let { move }: Props = $props();

	let plays = $state(0);

	function playAgain(): void {
		plays += 1;
	}
</script>

<div class="sheet" data-move-sheet={move}>
	<div class="stage">
		{#key plays}
			<div class="mover {move}" data-move={move}><code>{move}</code></div>
		{/key}
	</div>
	<button type="button" class="placed again" onclick={playAgain}>Play again</button>
</div>

<style>
	.sheet {
		display: grid;
		gap: var(--space-3);
		justify-items: start;
		padding: var(--space-3);
		border-radius: var(--radius-lg);
		background: var(--color-sheet);
		box-shadow: var(--elevation-sheet);
	}

	.stage {
		justify-self: stretch;
		padding: var(--space-4);
		overflow: hidden;
		border-radius: var(--radius-md);
		background: var(--color-surface);
	}

	.mover {
		padding: var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-note);
		box-shadow: var(--elevation-note);
	}

	.again {
		padding: var(--space-2) var(--space-3);
		border: 0;
		border-radius: var(--radius-md);
		background: var(--color-object);
		color: var(--color-text);
		font: inherit;
		cursor: pointer;
	}
</style>
