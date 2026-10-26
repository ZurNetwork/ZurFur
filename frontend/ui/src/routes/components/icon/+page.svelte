<script lang="ts">
	import { ICON_NAMES, Icon, type IconName } from '@zurfur/ui';
	import GalleryPage from '$gallery/GalleryPage.svelte';
	import GallerySkins from '$gallery/GallerySkins.svelte';
	import GalleryState from '$gallery/GalleryState.svelte';

	type Size = 'sm' | 'md' | 'lg';
	type Weight = 'regular' | 'heavy';

	interface SizeOption {
		readonly size: Size;
		readonly label: string;
		readonly token: string;
	}

	interface WeightOption {
		readonly weight: Weight;
		readonly label: string;
	}

	const sizeOptions: readonly SizeOption[] = [
		{ size: 'sm', label: 'Small', token: 'size-icon-sm' },
		{ size: 'md', label: 'Default', token: 'size-icon' },
		{ size: 'lg', label: 'Large', token: 'size-icon-lg' }
	];

	const weightOptions: readonly WeightOption[] = [
		{ weight: 'regular', label: 'Regular' },
		{ weight: 'heavy', label: 'Heavy' }
	];

	const checks: readonly IconName[] = ['check', 'circle-check', 'plus', 'x'];

	const group = $props.id();

	let size: Size = $state('md');
	let weight: Weight = $state('regular');
</script>

<GalleryPage>
	{#snippet states()}
		<GalleryState name="sizes">
			<h3>Sizes</h3>
			<GallerySkins>
				<ul class="row">
					{#each sizeOptions as option (option.size)}
						<li class="cell">
							<Icon name="image" size={option.size} />
							<code>{option.token}</code>
						</li>
					{/each}
				</ul>
			</GallerySkins>
		</GalleryState>
		<GalleryState name="weights">
			<h3>Weights</h3>
			<GallerySkins>
				<ul class="stack">
					{#each weightOptions as option (option.weight)}
						<li class="line">
							<span class="label">{option.label}</span>
							<span class="row">
								{#each checks as name (name)}
									<Icon {name} size="lg" weight={option.weight} />
								{/each}
							</span>
						</li>
					{/each}
				</ul>
			</GallerySkins>
		</GalleryState>
		<GalleryState name="beside-words">
			<h3>Beside Words</h3>
			<GallerySkins>
				<ul class="stack">
					<li class="line muted">
						<Icon name="clock" />
						<span>Due Friday</span>
					</li>
					<li class="line accent">
						<Icon name="circle-check" />
						<span>Sketch approved</span>
					</li>
					<li class="line">
						<Icon name="message-circle" />
						<span>3 replies from @artist.test</span>
					</li>
					<li class="line muted">
						<Icon name="lock" size="sm" />
						<span>Only you can see this</span>
					</li>
				</ul>
			</GallerySkins>
		</GalleryState>
	{/snippet}
	{#snippet example()}
		<div class="pickers">
			<fieldset class="picker">
				<legend>Size</legend>
				{#each sizeOptions as option (option.size)}
					<label>
						<input
							type="radio"
							name="{group}-size"
							value={option.size}
							checked={size === option.size}
							onchange={() => {
								size = option.size;
							}}
						/>
						{option.label}
					</label>
				{/each}
			</fieldset>
			<fieldset class="picker">
				<legend>Weight</legend>
				{#each weightOptions as option (option.weight)}
					<label>
						<input
							type="radio"
							name="{group}-weight"
							value={option.weight}
							checked={weight === option.weight}
							onchange={() => {
								weight = option.weight;
							}}
						/>
						{option.label}
					</label>
				{/each}
			</fieldset>
		</div>
		<ul class="grid">
			{#each ICON_NAMES as name (name)}
				<li class="tile">
					<Icon {name} {size} {weight} />
					<code>{name}</code>
				</li>
			{/each}
		</ul>
	{/snippet}
</GalleryPage>

<style>
	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	code {
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--space-6);
	}

	.cell {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
	}

	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	.line {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.label {
		min-width: var(--size-rail);
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	.muted {
		color: var(--color-text-muted);
	}

	.accent {
		color: var(--color-accent-text);
	}

	.pickers {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-8);
		margin-block-end: var(--space-4);
	}

	.picker {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
		margin: 0;
		padding: 0;
		border: none;
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	legend {
		float: left;
		padding: 0;
		margin-inline-end: var(--space-2);
		color: var(--color-text-muted);
	}

	label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	input {
		margin: 0;
		accent-color: var(--color-accent);
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(calc(var(--size-rail) * 2), 1fr));
		gap: var(--space-3);
	}

	.tile {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		min-height: var(--size-rail);
		padding: var(--space-3) var(--space-2);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-object);
		background: var(--color-surface);
		color: var(--color-text);
	}

	.tile code {
		color: var(--color-text-muted);
		overflow-wrap: anywhere;
		text-align: center;
	}
</style>
