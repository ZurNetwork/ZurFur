<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';
	import type { IconName } from './icons';

	/** Information, or something that went wrong. */
	type MessageTone = 'info' | 'error';

	/**
	 * A short message in a live status region. Keep it mounted and give it
	 * content when there is something to say: a screen reader announces
	 * what appears in a region that already exists.
	 */
	let {
		tone,
		testid,
		children
	}: {
		tone: MessageTone;
		testid?: string | undefined;
		children?: Snippet | undefined;
	} = $props();

	function toneIcon(messageTone: MessageTone): IconName {
		switch (messageTone) {
			case 'info':
				return 'info';
			case 'error':
				return 'circle-alert';
		}
	}
</script>

<div class="inline-message" role="status" data-testid={testid}>
	{#if children !== undefined}
		<div class="inline-message__box inline-message__box--{tone}" data-tone={tone}>
			<span class="inline-message__icon"><Icon name={toneIcon(tone)} /></span>
			<div class="inline-message__text">{@render children()}</div>
		</div>
	{/if}
</div>

<style>
	.inline-message__box {
		display: flex;
		align-items: flex-start;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-left: 3px solid;
		color: var(--color-text);
		font-size: var(--text-sm);
	}

	.inline-message__box--info {
		border-left-color: var(--color-accent);
	}

	.inline-message__box--info .inline-message__icon {
		color: var(--color-accent);
	}

	.inline-message__box--error {
		border-left-color: var(--color-danger);
	}

	.inline-message__box--error .inline-message__icon {
		color: var(--color-danger);
	}

	.inline-message__icon {
		display: inline-flex;
		padding-top: calc(var(--space-1) / 2);
	}

	.inline-message__text {
		min-width: 0;
		overflow-wrap: anywhere;
	}
</style>
