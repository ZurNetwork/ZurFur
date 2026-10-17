<script lang="ts">
	import type { TestStack } from '$lib/api/bug-report';
	import type { Session } from '$lib/api/session';
	import Button from '$lib/components/ui/Button.svelte';
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import { bugReportFromContext, snapshotItems } from './bug-report.svelte';
	import ReportBugForm from './ReportBugForm.svelte';

	/**
	 * "Report a bug" over any signed-in page, full screen on a phone: the
	 * shared form over the frame's draft and the snapshot read at opening,
	 * "Send report" and "Cancel" in the footer. Focus starts on "What
	 * happened?" (or on the sent message) and goes back to the control that
	 * opened it. `testStack` says whether the stack keeps reports. Outside the
	 * frame it renders nothing.
	 */
	let { session, buildId, testStack }: { session: Session; buildId: string; testStack: TestStack } =
		$props();

	const report = bugReportFromContext();
	const ownId = $props.id();
	const descriptionId = `${ownId}-description`;
	const sentId = `${ownId}-sent`;

	let formElement: HTMLFormElement | undefined = $state();

	const sent = $derived(report?.draft.status.status === 'sent');
	const sending = $derived(report?.draft.sending === true);
</script>

{#if report !== undefined}
	<Dialog
		bind:open={
			() => report.isOpen,
			(open: boolean) => {
				if (!open) report.close();
			}
		}
		label="Report a bug"
		variant="full-screen-on-phone"
		initialFocus={sent ? sentId : descriptionId}
		returnFocus={report.opener}
		{footer}
	>
		{#if report.snapshot !== undefined}
			<div class="report-bug-dialog">
				<ReportBugForm
					draft={report.draft}
					items={snapshotItems(report.snapshot, buildId)}
					{session}
					{testStack}
					{descriptionId}
					{sentId}
					bind:formElement
				/>
			</div>
		{/if}
	</Dialog>

	{#snippet footer()}
		{#if sent}
			<Button
				variant="quiet"
				onclick={() => {
					report.close();
				}}>Close</Button
			>
		{:else}
			<Button
				variant="quiet"
				onclick={() => {
					report.close();
				}}>Cancel</Button
			>
			<Button
				variant="primary"
				busy={sending}
				onclick={() => {
					formElement?.requestSubmit();
				}}>{sending ? 'Sending…' : 'Send report'}</Button
			>
		{/if}
	{/snippet}
{/if}

<style>
	.report-bug-dialog {
		padding: var(--space-4);
	}
</style>
