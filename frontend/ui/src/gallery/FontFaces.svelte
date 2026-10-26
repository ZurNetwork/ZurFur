<script lang="ts">
	import type { FontFile } from '@zurfur/ui';

	interface Props {
		readonly fonts: readonly FontFile[];
	}

	let { fonts }: Props = $props();

	type LoadState = 'unknown' | 'loaded' | 'not loaded';

	let states = $state<readonly LoadState[]>([]);

	/** The `font` shorthand that names a file's face at its lightest weight, as the package's fonts register it. */
	function shorthandOf(font: FontFile): string {
		const [lightest = '400'] = font.weight.split(' ');
		return `${font.style} ${lightest} 16px "${font.family} Variable"`;
	}

	/** Starts the face loading, then asks the browser whether it has it. */
	async function stateOf(font: FontFile): Promise<LoadState> {
		const shorthand = shorthandOf(font);
		const faces = await document.fonts.load(shorthand).catch(() => []);
		return faces.length > 0 && document.fonts.check(shorthand) ? 'loaded' : 'not loaded';
	}

	// The prerendered page cannot ask a browser, so it says unknown until it mounts.
	$effect(() => {
		let isCurrent = true;
		const wanted = fonts;
		void document.fonts.ready
			.then(() => Promise.all(wanted.map(stateOf)))
			.then((found) => {
				if (isCurrent) states = found;
			});
		return () => {
			isCurrent = false;
		};
	});
</script>

<ul class="faces">
	{#each fonts as font, index (index)}
		<li data-font={`${font.family} ${font.style}`}>
			<code>{font.family}</code>
			<code>{font.style} {font.weight}</code>
			<span data-font-state>{states[index] ?? 'unknown'}</span>
		</li>
	{/each}
</ul>

<style>
	.faces {
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
