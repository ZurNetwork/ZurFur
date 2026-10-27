<script lang="ts">
	import { useEnvironment } from '../environment/context';

	/** The ring with its words, announced as a status. */
	interface Status {
		readonly kind: 'status';
		/** What is loading, in words. */
		readonly label: string;
		/** Whether the words show beside the ring; hidden, they are still read out. */
		readonly labelShown?: boolean;
		readonly testid?: string | undefined;
	}

	/** The ring alone, for inside a button or a row that already says what is going on. */
	interface Glyph {
		readonly kind: 'glyph';
		readonly label?: undefined;
		readonly labelShown?: undefined;
		readonly testid?: string | undefined;
	}

	type Props = Status | Glyph;

	let { kind, label, labelShown = false, testid }: Props = $props();

	const { moves } = useEnvironment();
</script>

{#if kind === 'status'}
	<span class="zf-spinner-wrap" role="status" data-testid={testid}>
		<span class="zf-spinner" aria-hidden="true" {@attach moves.loop('spin')}></span>
		<span class={['zf-spinner-wrap__label', !labelShown && 'zf-sr']}>{label}</span>
	</span>
{:else}
	<span class="zf-spinner" aria-hidden="true" data-testid={testid} {@attach moves.loop('spin')}
	></span>
{/if}
