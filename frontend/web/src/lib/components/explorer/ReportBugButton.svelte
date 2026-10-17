<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import Icon from '$lib/components/ui/Icon.svelte';
	import { bugReportFromContext, REPORT_PAGE_DESCRIPTION_ID } from './bug-report.svelte';

	/**
	 * "Report a bug": a link to the report page that, once the script runs
	 * inside the frame, opens the dialog on a plain click instead (a modified
	 * or middle click still follows the link). On the report page itself it
	 * is marked current and focuses the page's text box.
	 */
	let { current }: { current: boolean } = $props();

	const report = bugReportFromContext();
	let scripted = $state(false);

	onMount(() => {
		scripted = true;
	});

	/** A click the browser would open elsewhere: a new tab, a new window, a download. */
	function modified(event: MouseEvent): boolean {
		return event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey;
	}

	function click(event: MouseEvent & { currentTarget: HTMLAnchorElement }): void {
		if (report === undefined || event.defaultPrevented || modified(event)) return;
		event.preventDefault();
		if (current) {
			document.getElementById(REPORT_PAGE_DESCRIPTION_ID)?.focus();
			return;
		}
		report.open(event.currentTarget);
	}
</script>

<a
	class="report-bug"
	href={resolve('/report-a-bug')}
	aria-haspopup={scripted && report !== undefined && !current ? 'dialog' : undefined}
	aria-current={current ? 'page' : undefined}
	onclick={click}
	data-testid="report-bug-link"
	><Icon name="bug" />
	<span>Report a bug</span></a
>

<style>
	.report-bug {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		text-decoration: none;
		white-space: nowrap;
	}

	.report-bug:hover,
	.report-bug[aria-current='page'] {
		color: var(--color-text);
	}
</style>
