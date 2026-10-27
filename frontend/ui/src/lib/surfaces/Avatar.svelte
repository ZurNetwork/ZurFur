<script lang="ts">
	import { badgeToneOf } from './choices';
	import { isImageSrc, type ImageSrc } from './image-src';
	import type { AvatarSize, AvatarTone } from './types';

	/** What every Avatar takes. */
	interface Common {
		/** The name from the profile, shown as text and never as markup. */
		readonly name: string;
		/** The picture; without one, or when it fails to load, the circle shows the name's first letter. */
		readonly picture?: ImageSrc | undefined;
		readonly size: AvatarSize;
		readonly tone: AvatarTone;
		readonly testid?: string | undefined;
	}

	/** The circle with the name, and the handle under it, beside it. */
	interface WithText extends Common {
		readonly textShown: true;
		/** The handle without its `@`, which the Avatar adds. */
		readonly handle?: string | undefined;
	}

	/** The circle alone, read out as the name. */
	interface Alone extends Common {
		readonly textShown: false;
		readonly handle?: never;
	}

	type Props = WithText | Alone;

	let { name, picture, size, tone, textShown, handle, testid }: Props = $props();

	/** U+2068 FIRST STRONG ISOLATE: opens the name's own direction run. */
	const FIRST_STRONG_ISOLATE = String.fromCodePoint(0x2068);

	/** U+2069 POP DIRECTIONAL ISOLATE: closes it. */
	const POP_DIRECTIONAL_ISOLATE = String.fromCodePoint(0x2069);

	/** The bidi controls, which could break the isolate around the name. */
	const BIDI_CONTROLS = /[\u202A-\u202E\u2066-\u2069]/gu;

	/** A grapheme that starts with a letter, a number or a pictograph. */
	const SHOWN_START = /^[\p{L}\p{N}\p{Extended_Pictographic}]/u;

	/** The size class: one of the three, otherwise `md`. */
	function sizeOf(value: unknown): AvatarSize {
		switch (value) {
			case 'sm':
				return 'sm';
			case 'lg':
				return 'lg';
			default:
				return 'md';
		}
	}

	/** The first whole grapheme that starts with a letter, a number or a pictograph, or nothing. */
	function initialOf(text: string): string | undefined {
		const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text);
		for (const { segment } of graphemes) {
			if (SHOWN_START.test(segment)) return segment;
		}
		return undefined;
	}

	/** The picture, checked again here; a value that fails the check shows the initial. */
	const checkedPicture = $derived(
		typeof picture === 'string' && isImageSrc(picture) ? picture : undefined
	);

	/** The picture that last failed to load; a new picture gets its own try. */
	let failedPicture = $state<string | undefined>();

	const shownPicture = $derived(checkedPicture === failedPicture ? undefined : checkedPicture);
	const shownName = $derived(typeof name === 'string' ? name : '');
	const initial = $derived(initialOf(shownName));
	const isolatedName = $derived(
		`${FIRST_STRONG_ISOLATE}${shownName.replace(BIDI_CONTROLS, '')}${POP_DIRECTIONAL_ISOLATE}`
	);
	/**
	 * Whether the circle is read out as the name: only alone, and only when the name has something
	 * to show. Beside its text, or with nothing to show, it is hidden.
	 */
	const isReadAsName = $derived(!textShown && initial !== undefined);
	const classes = $derived([
		'zf-avatar',
		`zf-avatar--${sizeOf(size)}`,
		`zf-avatar--${badgeToneOf(tone)}`
	]);

	/** The picture did not load: show the initial instead. */
	function fail(): void {
		failedPicture = shownPicture;
	}
</script>

<span class="zf-avatar-wrap" data-testid={testid}>
	<span
		class={classes}
		role={isReadAsName ? 'img' : undefined}
		aria-label={isReadAsName ? isolatedName : undefined}
		aria-hidden={isReadAsName ? undefined : 'true'}
	>
		{#if shownPicture !== undefined}
			<img src={shownPicture} alt="" loading="lazy" decoding="async" onerror={fail} />
		{:else if initial !== undefined}
			<bdi>{initial}</bdi>
		{/if}
	</span>
	{#if textShown}
		<span class="zf-avatar__text">
			<bdi class="zf-avatar__name">{shownName}</bdi>
			{#if typeof handle === 'string'}<bdi class="zf-handle">@{handle}</bdi>{/if}
		</span>
	{/if}
</span>
