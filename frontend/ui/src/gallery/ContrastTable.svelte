<script lang="ts">
	import {
		backgroundLabel,
		type ContrastReport,
		type PairKind,
		type RowResult
	} from './contrast/contrast-pairs';

	interface Props {
		readonly report: ContrastReport;
	}

	let { report }: Props = $props();

	function kindWords(kind: PairKind): string {
		switch (kind) {
			case 'text':
				return 'text';
			case 'largeText':
				return 'large text';
			case 'nonText':
				return 'non-text';
		}
	}

	function ratioWords(result: RowResult): string {
		switch (result.kind) {
			case 'measured':
				return result.ratio.toFixed(2);
			case 'unreadable':
				return result.reason;
		}
	}

	function verdictWords(result: RowResult): string {
		switch (result.kind) {
			case 'measured':
				return result.verdict;
			case 'unreadable':
				return 'unreadable';
		}
	}
</script>

<table>
	<thead>
		<tr>
			<th scope="col">Foreground</th>
			<th scope="col">Background</th>
			<th scope="col">Kind</th>
			<th scope="col">Ratio</th>
			<th scope="col">Minimum</th>
			<th scope="col">Result</th>
		</tr>
	</thead>
	<tbody>
		{#each report.rows as row, index (index)}
			{@const label = backgroundLabel(row.background)}
			<tr data-contrast-row="{row.foreground} {label}" data-verdict={verdictWords(row.result)}>
				<td><code>{row.foreground}</code></td>
				<td><code>{label}</code></td>
				<td>{kindWords(row.kind)}</td>
				<td>{ratioWords(row.result)}</td>
				<td>{row.minimum}</td>
				<td>{verdictWords(row.result)}</td>
			</tr>
		{/each}
	</tbody>
</table>

<style>
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	th,
	td {
		padding: var(--space-1) var(--space-2);
		border-bottom: 1px solid var(--color-border);
		text-align: start;
		vertical-align: top;
		overflow-wrap: anywhere;
	}

	th {
		font-weight: var(--weight-semibold);
	}

	tr[data-verdict='fail'] td:last-child,
	tr[data-verdict='unreadable'] td:last-child {
		color: var(--color-danger-text);
		font-weight: var(--weight-semibold);
	}
</style>
