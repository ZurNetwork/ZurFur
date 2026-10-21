<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import Icon from '$lib/components/ui/Icon.svelte';
	import {
		bugReportFromContext,
		NO_EXTRAS,
		REPORT_PAGE_DESCRIPTION_ID,
		type BugReportExtras
	} from './bug-report.svelte';

	/**
	 * "Report a bug": a link to the report page that, once the script runs
	 * inside the frame, opens the dialog on a plain click instead (a modified
	 * or middle click still follows the link). On the report page itself it
	 * is marked current and focuses the page's text box; with no box there (a
	 * report just sent) it is left to load the page afresh. `label` names it,
	 * `extras` are the items its opener adds, and `closeFirst` closes the
	 * modal it sits in (the drawer) before anything opens, answering the
	 * control focus goes back to when the dialog closes.
	 */
	let {
		current,
		label = 'Report a bug',
		extras = NO_EXTRAS,
		closeFirst
	}: {
		current: boolean;
		label?: string;
		extras?: BugReportExtras;
		closeFirst?: (() => Promise<HTMLElement | undefined>) | undefined;
	} = $props();

	const report = bugReportFromContext();
	let scripted = $state(false);

	onMount(() => {
		scripted = true;
	});

	/** A click the browser would open elsewhere: a new tab, a new window, a download. */
	function modified(event: MouseEvent): boolean {
		return event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey;
	}

	/** Open the dialog (or, on the report page, focus its text box), giving focus back to `returnTo`. */
	function openFrom(returnTo: HTMLElement): void {
		if (current) {
			document.getElementById(REPORT_PAGE_DESCRIPTION_ID)?.focus();
			return;
		}
		report?.open(returnTo, extras);
	}

	/** Close the modal the link sits in, then open from the control it answers (else the link). */
	async function openAfterClosing(
		close: () => Promise<HTMLElement | undefined>,
		clicked: HTMLElement
	): Promise<void> {
		const returnTo = await close();
		openFrom(returnTo ?? clicked);
	}

	/** On the report page with no text box (the sent message shows): the link loads a fresh form. */
	function loadsAfresh(): boolean {
		return (
			current && (document.getElementById(REPORT_PAGE_DESCRIPTION_ID) ?? undefined) === undefined
		);
	}

	function click(event: MouseEvent & { currentTarget: HTMLAnchorElement }): void {
		if (report === undefined || event.defaultPrevented || modified(event)) return;
		if (loadsAfresh()) {
			void closeFirst?.();
			return;
		}
		event.preventDefault();
		if (closeFirst === undefined) {
			openFrom(event.currentTarget);
			return;
		}
		void openAfterClosing(closeFirst, event.currentTarget);
	}
</script>

<a
	class="report-bug"
	href={resolve('/report-a-bug')}
	aria-haspopup={scripted && report !== undefined && !current ? 'dialog' : undefined}
	aria-current={current ? 'page' : undefined}
	data-sveltekit-reload={current ? '' : undefined}
	onclick={click}
	data-testid="report-bug-link"
	><Icon name="bug" />
	<span>{label}</span></a
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
