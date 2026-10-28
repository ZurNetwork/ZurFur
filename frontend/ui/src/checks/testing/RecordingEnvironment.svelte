<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { provideEnvironment, type Environment } from '../../lib/environment/context.ts';
	import type { LoopKind, Moves, NoteOrigin } from '../../lib/environment/moves.ts';
	import type { SoundCue, Sounds } from '../../lib/environment/sounds.ts';

	/** A move, with the arguments it was called with; `life`'s is its `onEnd`, so a spec can end the life. */
	type MoveCall =
		| { readonly move: 'squish' | 'dim' | 'burst'; readonly args: readonly [] }
		| { readonly move: 'note'; readonly args: readonly [origin: NoteOrigin] }
		| { readonly move: 'loop'; readonly args: readonly [kind: LoopKind] }
		| { readonly move: 'life'; readonly args: readonly [onEnd: () => void] };

	/** One call into the seam: a move attached to an element, or a cue played. */
	type SeamCall =
		| (MoveCall & { readonly kind: 'move'; readonly element: HTMLElement })
		| { readonly kind: 'sound'; readonly cue: SoundCue };

	interface Props {
		/** Called once for every move attached and every cue played below. */
		readonly onRecord: (call: SeamCall) => void;
		readonly children: Snippet;
	}

	let { onRecord, children }: Props = $props();

	/**
	 * An attachment that records the move, with its arguments, against the element
	 * it lands on. The record is untracked, so a re-render that hands the wrapper
	 * a new `onRecord` never re-runs the attachment and records it twice.
	 */
	function recorded(call: MoveCall): Attachment<HTMLElement> {
		return (element) => {
			untrack(() => {
				onRecord({ kind: 'move', element, ...call });
			});
		};
	}

	const moves: Moves = Object.freeze({
		squish: () => recorded({ move: 'squish', args: [] }),
		note: (origin: NoteOrigin) => recorded({ move: 'note', args: [origin] }),
		dim: () => recorded({ move: 'dim', args: [] }),
		life: (onEnd: () => void) => recorded({ move: 'life', args: [onEnd] }),
		loop: (kind: LoopKind) => recorded({ move: 'loop', args: [kind] }),
		burst: () => recorded({ move: 'burst', args: [] })
	});

	const sounds: Sounds = Object.freeze({
		play: (cue: SoundCue) => {
			onRecord({ kind: 'sound', cue });
		}
	});

	const environment: Environment = Object.freeze({ moves, sounds });

	provideEnvironment(environment);
</script>

{@render children()}
