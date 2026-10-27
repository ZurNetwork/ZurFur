<script lang="ts">
	import { useEnvironment } from '../environment/context';

	/**
	 * The status: always on the page, holding the ring and its words only while
	 * active. A page mounts it once and changes `active`, so the words are announced.
	 */
	interface Status {
		readonly kind: 'status';
		/** What is loading, in words. */
		readonly label: string;
		/** Whether it is loading: the ring and the words show only while this is `true`. */
		readonly active: boolean;
		/** Whether the words show beside the ring; hidden, they are still read out. */
		readonly labelShown?: boolean;
		readonly testid?: string | undefined;
	}

	/** The ring alone, for inside a button or a row that already says what is going on. */
	interface Glyph {
		readonly kind: 'glyph';
		readonly label?: undefined;
		readonly active?: undefined;
		readonly labelShown?: undefined;
		readonly testid?: string | undefined;
	}

	type Props = Status | Glyph;

	let { kind, label, active, labelShown = false, testid }: Props = $props();

	const { moves } = useEnvironment();

	/** Whether the status is loading: exactly `true`, anything else is not. */
	function isActive(value: unknown): boolean {
		return value === true;
	}
</script>

{#if kind === 'status'}
	<span class="zf-spinner-wrap" role="status" data-testid={testid}>
		{#if isActive(active)}
			<span class="zf-spinner" aria-hidden="true" {@attach moves.loop('spin')}></span>
			<span class={['zf-spinner-wrap__label', !labelShown && 'zf-sr']}>{label}</span>
		{/if}
	</span>
{:else}
	<span class="zf-spinner" aria-hidden="true" data-testid={testid} {@attach moves.loop('spin')}
	></span>
{/if}
