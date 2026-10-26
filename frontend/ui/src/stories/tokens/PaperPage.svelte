<script lang="ts">
	import { PAPER_MOVES, type Skin } from '@zurfur/ui';
	import GalleryPage from '../../gallery/GalleryPage.svelte';
	import GallerySkins from '../../gallery/GallerySkins.svelte';
	import GalleryState from '../../gallery/GalleryState.svelte';
	import MoveSheet from '../../gallery/MoveSheet.svelte';

	interface Props {
		/** Shows only this skin, as the toolbar picks it; unset shows both side by side. */
		readonly skin?: Skin | undefined;
	}

	let { skin }: Props = $props();
</script>

<GalleryPage framed={skin !== undefined}>
	{#snippet states()}
		<GalleryState name="desk">
			<h3>Desk</h3>
			<GallerySkins only={skin}>
				<div class="paper desk">
					<div class="sheet">
						<p class="labels">
							<code>color-sheet</code>
							<code>radius-sheet</code>
							<code>elevation-sheet</code>
						</p>
						<div class="note">
							<p class="labels"><code>color-note</code> <code>elevation-note</code></p>
						</div>
					</div>
				</div>
			</GallerySkins>
		</GalleryState>
		<GalleryState name="objects">
			<h3>Objects</h3>
			<GallerySkins only={skin}>
				<div class="tiles">
					<div class="obj tile"><code>obj</code></div>
					<div class="obj lift tile"><code>lift</code></div>
					<div class="lifts">
						<div class="obj lift-art tile"><code>lifts</code> <code>lift-art</code></div>
					</div>
					<button type="button" class="placed tile"><code>placed</code></button>
					<button type="button" class="placed glide tile"><code>glide</code></button>
					<button type="button" class="placed glidex tile"><code>glidex</code></button>
					<button type="button" class="placed seg tile"><code>seg</code></button>
					<button type="button" class="placed glidebox tile"><code>glidebox</code></button>
					<div class="slip tile"><code>slip</code></div>
				</div>
			</GallerySkins>
		</GalleryState>
		<GalleryState name="piece">
			<h3>Piece</h3>
			<GallerySkins only={skin}>
				<div class="piece rows">
					<div class="pick row"><code>pick</code></div>
					<div role="listbox" aria-label="Selected row">
						<div class="pick row" role="option" aria-selected="true">
							<code>aria-selected</code>
						</div>
					</div>
					<button type="button" class="pick row" aria-pressed="true">
						<code>aria-pressed</code>
					</button>
					<div class="pick row" aria-current="true"><code>aria-current</code></div>
					<div class="pick row is-on"><code>is-on</code></div>
					<div class="pick row is-locked"><code>is-locked</code></div>
				</div>
			</GallerySkins>
		</GalleryState>
		<GalleryState name="piles">
			<h3>Piles</h3>
			<GallerySkins only={skin}>
				<div class="piles">
					<div class="pile">
						<div class="peek pk1"></div>
						<div class="top"><code>pk1</code></div>
					</div>
					<div class="pile">
						<div class="peek pk1"></div>
						<div class="peek pk2"></div>
						<div class="top"><code>pk1</code> <code>pk2</code></div>
					</div>
					<div class="pile">
						<div class="peek pk1"></div>
						<div class="peek pk2"></div>
						<div class="peek pk3"></div>
						<div class="top"><code>pk1</code> <code>pk2</code> <code>pk3</code></div>
					</div>
				</div>
			</GallerySkins>
		</GalleryState>
		<GalleryState name="moves">
			<h3>Moves</h3>
			<GallerySkins only={skin}>
				<div class="moves">
					{#each PAPER_MOVES as move (move)}
						<MoveSheet {move} />
					{/each}
				</div>
			</GallerySkins>
		</GalleryState>
	{/snippet}
</GalleryPage>

<style>
	.desk {
		padding: var(--space-desk);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		background: var(--color-bg);
	}

	.sheet {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-4);
		border-radius: var(--radius-sheet);
		background: var(--color-sheet);
		box-shadow: var(--elevation-sheet);
	}

	.note {
		padding: var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-note);
		box-shadow: var(--elevation-note);
	}

	.labels {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin: 0;
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	.tiles {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
		align-items: flex-start;
	}

	.tile {
		padding: var(--space-3);
		border: 0;
		border-radius: var(--radius-object);
		color: var(--color-text);
		font: inherit;
	}

	button.tile {
		background: var(--color-object);
		cursor: pointer;
	}

	.rows {
		display: grid;
		gap: var(--space-1);
		padding: var(--space-2);
	}

	.row {
		padding: var(--space-2) var(--space-3);
		border: 0;
		color: var(--color-text);
		font: inherit;
		text-align: start;
	}

	.row.is-locked {
		color: var(--color-text-muted);
	}

	.piles {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--space-6);
		padding-bottom: var(--space-6);
	}

	.pile {
		border-radius: var(--radius-sheet);
	}

	.top {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		padding: var(--space-4);
		border-radius: var(--radius-sheet);
		background: var(--color-pile-top);
		box-shadow: var(--elevation-sheet);
		font-size: var(--text-sm);
	}

	.moves {
		display: grid;
		gap: var(--space-4);
	}
</style>
