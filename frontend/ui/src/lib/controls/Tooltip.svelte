<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { createAttachmentKey, type Attachment } from 'svelte/attachments';
	import { portalTarget } from './floating';

	/** What a trigger is handed: the attributes and handlers to spread on its element. */
	interface TriggerArgs {
		readonly props: Record<string, unknown>;
	}

	interface Props {
		/** The tip's words: the same words that name the trigger, shown, never read twice. */
		readonly words: string;
		/** The trigger: one element that spreads `props`. */
		readonly trigger: Snippet<[TriggerArgs]>;
		/** A press on the trigger. */
		readonly onPress: () => void;
		readonly disabled?: boolean;
	}

	let { words, trigger, onPress, disabled = false }: Props = $props();

	/** The trigger's element, once it is mounted. */
	let anchor: HTMLElement | undefined = $state();
	const portalTo = $derived(anchor === undefined ? undefined : portalTarget(anchor));

	/** Keeps the trigger's element while it is mounted, so the tip knows which skin it opens in. */
	const remember: Attachment<HTMLElement> = (element) => {
		anchor = element;
		return () => {
			anchor = undefined;
		};
	};
	const rememberKey = createAttachmentKey();

	/**
	 * The trigger's props, less the description bits-ui adds while the tip shows
	 * (the tip is visual only: the trigger's name already says it), plus `remember`.
	 */
	function triggerProps(props: Record<string, unknown>): Record<string, unknown> {
		const kept: Record<string | symbol, unknown> = { ...props, [rememberKey]: remember };
		delete kept['aria-describedby'];
		return kept;
	}
</script>

<Tooltip.Provider>
	<Tooltip.Root>
		<Tooltip.Trigger
			{disabled}
			onclick={() => {
				onPress();
			}}
		>
			{#snippet child({ props }: TriggerArgs)}
				{@render trigger({ props: triggerProps(props) })}
			{/snippet}
		</Tooltip.Trigger>
		{#if portalTo !== undefined}
			<Tooltip.Portal to={portalTo}>
				<Tooltip.Content>
					{#snippet child({ props, wrapperProps })}
						<div {...wrapperProps}>
							<div {...props} class="zf-tip" aria-hidden="true">{words}</div>
						</div>
					{/snippet}
				</Tooltip.Content>
			</Tooltip.Portal>
		{/if}
	</Tooltip.Root>
</Tooltip.Provider>
