<script lang="ts">
	import { avatarSizeOf, badgeToneOf, initialOf } from './choices';
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

	/** Everything that draws nothing: bidi controls, default-ignorable code points and white space. */
	const INVISIBLE = /[\p{Bidi_Control}\p{Default_Ignorable_Code_Point}\p{White_Space}]/gu;

	/** The picture, checked again here; a value that fails the check shows the initial. */
	const checkedPicture = $derived(
		typeof picture === 'string' && isImageSrc(picture) ? picture : undefined
	);

	/** The picture that last failed to load; a new picture gets its own try. */
	let failedPicture = $state<string | undefined>();

	const shownPicture = $derived(checkedPicture === failedPicture ? undefined : checkedPicture);
	const shownName = $derived(typeof name === 'string' ? name : '');
	const initial = $derived(initialOf(shownName));
	const heardName = $derived(shownName.replace(BIDI_CONTROLS, ''));
	const isolatedName = $derived(`${FIRST_STRONG_ISOLATE}${heardName}${POP_DIRECTIONAL_ISOLATE}`);
	/**
	 * Whether the circle is read out as the name: only alone, and only when the name, without its
	 * invisible characters and white space, isn't empty. Beside its text, or with no name, it is hidden.
	 */
	const isReadAsName = $derived(!textShown && shownName.replace(INVISIBLE, '') !== '');
	const classes = $derived([
		'zf-avatar',
		`zf-avatar--${avatarSizeOf(size)}`,
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
