<script lang="ts">
	import type { MoveName } from '../lib/environment/moves.ts';
	import type { SoundCue } from '../lib/environment/sounds.ts';

	interface Props {
		/** The moves the control calls. */
		readonly moves: readonly MoveName[];
		/** The cues the control plays. */
		readonly sounds: readonly SoundCue[];
	}

	let { moves, sounds }: Props = $props();

	/** What each move looks like, in a line. */
	const moveWords: Readonly<Record<MoveName, string>> = {
		squish: 'Squishes a little when pressed, then springs back',
		note: 'Put down like a note, picked up when it closes',
		dim: 'Dims the desk behind it, clears when it closes',
		life: 'Lands from below, waits, then goes by itself',
		loop: 'Spins or breathes for as long as it is there',
		burst: 'Throws a little confetti, once'
	};

	/** What each cue sounds like, in a line. */
	const soundWords: Readonly<Record<SoundCue, string>> = {
		open: 'A short middle blip',
		close: 'A short low blip',
		toggle: 'A short high blip',
		celebrate: 'A short C-E-G chord'
	};
</script>

<div class="motion">
	<div class="part">
		<h3>Moves</h3>
		{#if moves.length === 0}
			<p class="none">None</p>
		{:else}
			<dl>
				{#each moves as move (move)}
					<dt><code>{move}</code></dt>
					<dd>{moveWords[move]}</dd>
				{/each}
			</dl>
		{/if}
	</div>
	<div class="part">
		<h3>Sounds</h3>
		{#if sounds.length === 0}
			<p class="none">None</p>
		{:else}
			<dl>
				{#each sounds as cue (cue)}
					<dt><code>{cue}</code></dt>
					<dd>{soundWords[cue]}</dd>
				{/each}
			</dl>
		{/if}
	</div>
	<p class="still">The gallery stays still and silent until the motion layer arrives.</p>
</div>

<style>
	.motion {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--size-dialog)), 1fr));
		gap: var(--space-4) var(--space-8);
	}

	h3 {
		margin: 0 0 var(--space-2);
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
		font-weight: var(--weight-semibold);
	}

	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: var(--space-2) var(--space-4);
		margin: 0;
	}

	dt,
	dd {
		margin: 0;
	}

	code {
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	.none {
		margin: 0;
		color: var(--color-text-muted);
	}

	.still {
		grid-column: 1 / -1;
		margin: 0;
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}
</style>
