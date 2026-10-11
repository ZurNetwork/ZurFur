<script lang="ts">
	import type { DenLevel } from '$lib/api/den';
	import Icon from './Icon.svelte';
	import type { IconName } from './icons';

	/**
	 * A node's own visibility: an icon and its word, never colour alone. An
	 * unknown level shows nothing — never "Public".
	 */
	let { level }: { level: DenLevel } = $props();

	/** The look of each known level. */
	const LOOKS = {
		private: { icon: 'lock', word: 'Private' },
		listed: { icon: 'eye', word: 'Listed' },
		public: { icon: 'globe', word: 'Public' }
	} as const satisfies Record<Exclude<DenLevel, 'unknown'>, { icon: IconName; word: string }>;
</script>

{#if level !== 'unknown'}
	{@const look = LOOKS[level]}
	<span class="visibility visibility--{level}" data-testid="visibility">
		<Icon name={look.icon} />
		<span>{look.word}</span>
	</span>
{/if}

<style>
	.visibility {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		font-size: var(--text-xs);
		white-space: nowrap;
	}

	.visibility--private {
		color: var(--color-visibility-private);
	}

	.visibility--listed {
		color: var(--color-visibility-listed);
	}

	.visibility--public {
		color: var(--color-visibility-public);
	}
</style>
