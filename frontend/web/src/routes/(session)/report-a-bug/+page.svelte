<script lang="ts">
	import type { BugReportOffer, BugReportPageOffer } from '$lib/api/bug-report';
	import {
		BugReportDraft,
		draftStartOf,
		offerItems,
		REPORT_PAGE_DESCRIPTION_ID
	} from '$lib/components/explorer/bug-report.svelte';
	import ReportBugForm from '$lib/components/explorer/ReportBugForm.svelte';
	import type { PageProps } from './$types';

	/**
	 * The report page: the shared form over what the load offers, working
	 * without script. Without script a post answers this page again, and the
	 * form starts from that answer: the reference, or the reporter's own text
	 * and why it was refused, the page they sent back in "Which page?" to
	 * change or empty. With script it posts in place.
	 */
	let { data, form }: PageProps = $props();

	const sentId = $props.id();
	const ASK: BugReportPageOffer = { kind: 'ask' };

	// svelte-ignore state_referenced_locally
	const start = draftStartOf(form);
	const draft = new BugReportDraft(start);
	const askPage = $derived(data.offer.page.kind === 'ask' || start.status.status === 'refused');
	const offer: BugReportOffer = $derived(askPage ? { ...data.offer, page: ASK } : data.offer);
	const items = $derived(offerItems(offer, data.buildId));
</script>

<svelte:head>
	<title>Report a bug · Zurfur</title>
</svelte:head>

<section class="report">
	<h1>Report a bug</h1>
	<ReportBugForm
		{draft}
		{items}
		session={data.session}
		descriptionId={REPORT_PAGE_DESCRIPTION_ID}
		{sentId}
		{askPage}
		submitButton
	/>
</section>

<style>
	.report {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-width: 40rem;
	}
</style>
