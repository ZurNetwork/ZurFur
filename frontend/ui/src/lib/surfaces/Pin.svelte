<script lang="ts">
	import { useEnvironment } from '../environment/context';
	import { pinPointFrom, type PinPoint } from './pin-point';

	interface Props {
		/** Where the pin sits on the art; a point that fails the check draws no pin. */
		readonly point: PinPoint;
		/** The short text in the circle, such as the pin's number. */
		readonly mark: string;
		/** The button's name, read out instead of the mark; it should contain the mark. */
		readonly label: string;
		/** Whether the thread the pin opens is showing. */
		readonly open: boolean;
		/** The id of the thread the pin opens. */
		readonly controls?: string | undefined;
		readonly onPress: () => void;
		readonly testid?: string | undefined;
	}

	let { point, mark, label, open, controls, onPress, testid }: Props = $props();

	const { moves } = useEnvironment();

	/** The point, checked again here and copied, so only its two numbers reach the page. */
	const shownPoint = $derived(pinPointFrom(point));

	/** Whether the pin is open: exactly `true`, anything else is closed. */
	function isOpen(value: unknown): boolean {
		return value === true;
	}

	/** A fraction as a percentage of the art's width or height. */
	function percentOf(fraction: number): string {
		return `${String(fraction * 100)}%`;
	}
</script>

{#if shownPoint !== undefined}
	<button
		type="button"
		class={['zf-pin', isOpen(open) && 'is-active']}
		style:left={percentOf(shownPoint.x)}
		style:top={percentOf(shownPoint.y)}
		aria-label={label}
		aria-expanded={isOpen(open) ? 'true' : 'false'}
		aria-controls={controls}
		data-testid={testid}
		onclick={() => {
			onPress();
		}}
		{@attach moves.squish()}
	>
		{mark}
	</button>
{/if}
