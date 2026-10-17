<script lang="ts">
	import type { Snippet } from 'svelte';
	import { BUG_REPORT_FIELDS, type BugReportFieldName } from '$lib/api/bug-report';
	import Checkbox from '$lib/components/ui/Checkbox.svelte';
	import type { BugReportDraft, SentItem, TickItem } from './bug-report.svelte';

	/**
	 * "Sent with your report": one tick box per item, its exact value beneath
	 * its name. An unticked value is greyed and says "Not sent", and none of
	 * its fields is posted. `before` renders first inside the group.
	 */
	let {
		draft,
		items,
		before
	}: {
		draft: BugReportDraft;
		items: readonly SentItem[];
		before?: Snippet | undefined;
	} = $props();

	/** What a tick box is described by: its value, or "Not sent" before the greyed value. */
	function described(item: SentItem, ticked: boolean): string {
		return ticked ? item.value : `Not sent · ${item.value}`;
	}

	/** The tick box's own field name. */
	function boxName(item: TickItem): BugReportFieldName {
		switch (item) {
			case 'page':
				return BUG_REPORT_FIELDS.sendPage;
			case 'pageType':
				return BUG_REPORT_FIELDS.sendPageType;
			case 'webVersion':
				return BUG_REPORT_FIELDS.sendWebVersion;
			case 'browser':
				return BUG_REPORT_FIELDS.sendBrowser;
			case 'window':
				return BUG_REPORT_FIELDS.sendWindow;
			case 'display':
				return BUG_REPORT_FIELDS.sendDisplay;
		}
	}
</script>

<fieldset class="sent-with-report">
	<legend class="sent-with-report__legend">Sent with your report</legend>
	{@render before?.()}
	{#each items as item (item.item)}
		{@const ticked = draft.ticks[item.item]}
		<div class="sent-with-report__item" data-testid="sent-item-{item.item}">
			<Checkbox
				label={item.label}
				description={described(item, ticked)}
				name={boxName(item.item)}
				bind:checked={
					() => draft.ticks[item.item],
					(value: boolean) => {
						draft.setTick(item.item, value);
					}
				}
			/>
			{#if item.note !== undefined}
				<p class="sent-with-report__note">{item.note}</p>
			{/if}
			{#if ticked}
				{#each item.fields as field (field.name)}
					<input type="hidden" name={field.name} value={field.value} />
				{/each}
			{/if}
		</div>
	{/each}
</fieldset>

<style>
	.sent-with-report {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
	}

	.sent-with-report__legend {
		margin-bottom: var(--space-2);
		padding: 0;
		color: var(--color-text);
		font-size: var(--text-sm);
		font-weight: var(--text-weight-medium);
	}

	.sent-with-report__item {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	/* The value beneath each name, exactly as sent: monospace, wrapping anywhere. */
	.sent-with-report__item :global(.checkbox__description) {
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}

	.sent-with-report__note {
		margin: 0 0 0 calc(var(--space-5) + var(--space-2));
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}
</style>
