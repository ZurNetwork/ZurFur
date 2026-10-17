<script lang="ts">
	import type { SubmitFunction } from '@sveltejs/kit';
	import { enhance } from '$app/forms';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { BUG_REPORT_FIELDS, TICKED } from '$lib/api/bug-report';
	import type { Session } from '$lib/api/session';
	import Button from '$lib/components/ui/Button.svelte';
	import InlineMessage from '$lib/components/ui/InlineMessage.svelte';
	import TextInput from '$lib/components/ui/TextInput.svelte';
	import { codePointCount, DESCRIPTION_MAX_CODE_POINTS } from '$lib/types/bug-report-rules';
	import {
		COUNT_SHOWN_FROM,
		outcomeOf,
		problemMessage,
		type BugReportDraft,
		type ProblemMessage,
		type SentItem
	} from './bug-report.svelte';
	import SentWithReport from './SentWithReport.svelte';

	/**
	 * The bug-report form, shared by the dialog and the report page: "What
	 * happened?", what is sent with it, and who it is sent as. It posts the
	 * report page's default action with plain `enhance`, settling `draft` from
	 * the answer (sent, refused by kind, couldn't send, or a redirect followed)
	 * and never touching the page's own data. A sent report replaces the form
	 * with its reference. `askPage` adds the optional "Which page?" box;
	 * `submitButton` puts "Send report" inside the form (the page), else the
	 * dialog's footer submits `formElement`.
	 */
	let {
		draft,
		items,
		session,
		descriptionId,
		sentId,
		askPage = false,
		submitButton = false,
		formElement = $bindable()
	}: {
		draft: BugReportDraft;
		items: readonly SentItem[];
		session: Session;
		descriptionId: string;
		sentId: string;
		askPage?: boolean;
		submitButton?: boolean;
		formElement?: HTMLFormElement | undefined;
	} = $props();

	const status = $derived(draft.status);
	const message: ProblemMessage | undefined = $derived(
		status.status === 'refused' ? problemMessage(status.problem, askPage) : undefined
	);
	const fieldError = $derived(message?.place === 'field' ? message.text : undefined);
	const statusMessage = $derived(message?.place === 'status' ? message : undefined);
	const count = $derived(codePointCount(draft.description));
	const sentAs = $derived(session.handle === undefined ? session.did : `@${session.handle}`);

	let sentMessage: HTMLElement | undefined = $state();

	// A sent report moves focus to its message; an empty description to its box.
	$effect(() => {
		if (status.status === 'sent') {
			sentMessage?.focus();
		} else if (status.status === 'refused' && status.problem.kind === 'descriptionEmpty') {
			document.getElementById(descriptionId)?.focus();
		}
	});

	/** Post once at a time, and settle the draft from whatever the action answers. */
	const submit: SubmitFunction = ({ cancel }) => {
		const ticket = draft.beginSend();
		if (ticket === undefined) {
			cancel();
			return;
		}
		return async ({ result }) => {
			const outcome = outcomeOf(result);
			draft.settle(ticket, outcome);
			switch (outcome.kind) {
				case 'sent':
				case 'refused':
					return;
				case 'redirect':
					// eslint-disable-next-line svelte/no-navigation-without-resolve -- the action's own redirect, an app path (an ended session goes to /login)
					await goto(outcome.location);
					return;
			}
		};
	};
</script>

<div class="report-form">
	<div class="visually-hidden" aria-live="polite" data-testid="report-announcer">
		{status.status === 'sent' ? 'Report sent.' : ''}
	</div>

	{#if status.status === 'sent'}
		<div class="report-form__sent" id={sentId} tabindex="-1" bind:this={sentMessage}>
			<p>Thanks. Your report is saved.</p>
			<p>Reference: <code data-testid="report-reference">{status.reference}</code></p>
		</div>
	{:else}
		<form
			method="POST"
			action={resolve('/report-a-bug')}
			class="report-form__form"
			aria-busy={draft.sending ? 'true' : undefined}
			bind:this={formElement}
			use:enhance={submit}
		>
			<div class="report-form__description">
				<TextInput
					id={descriptionId}
					label="What happened?"
					hint="What did you do, what did you expect, and what happened instead?"
					multiline
					required
					name={BUG_REPORT_FIELDS.description}
					error={fieldError}
					bind:value={draft.description}
				/>
				{#if count >= COUNT_SHOWN_FROM}
					<span class="report-form__count" data-testid="report-count"
						>{count.toLocaleString('en-GB')} / {DESCRIPTION_MAX_CODE_POINTS.toLocaleString(
							'en-GB'
						)}</span
					>
				{/if}
			</div>

			<SentWithReport {draft} {items} before={askPage ? whichPage : undefined} />

			<div class="report-form__fixed">
				<p>Sent as {sentAs}. Zurfur also records when it arrives.</p>
				<p>
					Never sent: your cookies, sign-in tokens, other pages you visited, or what's on your
					screen.
				</p>
			</div>

			<InlineMessage
				tone={statusMessage?.tone ?? 'info'}
				testid="report-status"
				children={statusMessage === undefined ? undefined : statusText}
			/>

			{#if submitButton}
				<div>
					<Button type="submit" variant="primary" busy={draft.sending}
						>{draft.sending ? 'Sending…' : 'Send report'}</Button
					>
				</div>
			{/if}
		</form>
	{/if}
</div>

{#snippet statusText()}
	{statusMessage?.text}
{/snippet}

{#snippet whichPage()}
	<input type="hidden" name={BUG_REPORT_FIELDS.sendPage} value={TICKED} />
	<TextInput label="Which page?" name={BUG_REPORT_FIELDS.page} bind:value={draft.pageText} />
{/snippet}

<style>
	.report-form {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 0;
	}

	.report-form__form {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		min-width: 0;
	}

	.report-form__description {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	/* The count only ever informs: muted, never a warning colour. */
	.report-form__count {
		align-self: flex-end;
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	.report-form__fixed {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	.report-form__fixed p:last-child,
	.report-form__sent p:last-child {
		margin-bottom: 0;
	}

	.report-form__sent:focus {
		outline: none;
	}

	.report-form__sent code {
		overflow-wrap: anywhere;
	}
</style>
