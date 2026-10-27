/**
 * The moves contract as a suite: call `movesContract` at the top of a spec file.
 * With no adapter it holds the still moves: every attachment touches nothing and
 * `life` never calls `onEnd`. With an adapter it registers the adapter's rows
 * instead: run those from a browser spec, once per mode, each time with a fresh
 * adapter set to that mode, short token values on the containers `mount` hands
 * out, and the adapter's injected clock behind `advance`.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Attachment } from 'svelte/attachments';
import type { LoopKind, MoveName, Moves, NoteOrigin } from '../lib/environment/moves.ts';

/** The four modes an adapter runs the suite in. */
export type ContractMode = 'lively' | 'still' | 'reduced' | 'game-animations-off';

/** What the suite needs from an adapter's spec to run the adapter's rows. */
export interface ContractAdapter {
	/** The mode the adapter under test is set to. */
	readonly mode: ContractMode;
	/**
	 * A fresh container in the document for one row: inside whatever the mode needs
	 * (a still ancestor, emulated reduced motion) and carrying short token values.
	 * The suite removes it after the row.
	 */
	readonly mount: () => HTMLElement;
	/** Moves the adapter's clock on by `ms`, then lets everything that came due run. */
	readonly advance: (ms: number) => Promise<void>;
	/** The shortest a life may last in this mode under the container's values; `onEnd` never comes sooner. */
	readonly lifeMs: number;
	/** A margin longer than any one-shot move under those values; a due `onEnd` comes within it. */
	readonly settleMs: number;
}

/** What `movesContract` runs: the adapter's rows when there is one, the still rows when not. */
export interface ContractOptions {
	readonly adapter: ContractAdapter | undefined;
}

/** Every key of a record that names each member of `K`, so a new member must be listed. */
function keysOf<K extends string>(record: Readonly<Record<K, true>>): readonly K[] {
	return Object.keys(record) as K[];
}

const ORIGINS = keysOf<NoteOrigin>({
	'in-place': true,
	'edge-start': true,
	'edge-end': true,
	below: true
});

const LOOP_KINDS = keysOf<LoopKind>({ spin: true, breathe: true });

/** Every way to call each move, with every literal argument it takes. */
const EVERY_CALL: Readonly<
	Record<MoveName, (moves: Moves, onEnd: () => void) => readonly Attachment<HTMLElement>[]>
> = {
	squish: (moves) => [moves.squish()],
	note: (moves) => ORIGINS.map((origin) => moves.note(origin)),
	dim: (moves) => [moves.dim()],
	life: (moves, onEnd) => [moves.life(onEnd)],
	loop: (moves) => LOOP_KINDS.map((kind) => moves.loop(kind)),
	burst: (moves) => [moves.burst()]
};

/** A stand-in element that records every way anything reaches into it, and refuses each. */
function refusingElement(touched: string[]): HTMLElement {
	const handler: ProxyHandler<object> = {
		get(target, key) {
			touched.push(`get ${String(key)}`);
			const found: unknown = Reflect.get(target, key);
			return found;
		},
		set(_, key) {
			touched.push(`set ${String(key)}`);
			return false;
		},
		has(_, key) {
			touched.push(`has ${String(key)}`);
			return false;
		},
		deleteProperty(_, key) {
			touched.push(`delete ${String(key)}`);
			return false;
		},
		defineProperty(_, key) {
			touched.push(`define ${String(key)}`);
			return false;
		},
		getOwnPropertyDescriptor(_, key) {
			touched.push(`describe ${String(key)}`);
			return undefined;
		},
		ownKeys() {
			touched.push('ownKeys');
			return [];
		},
		getPrototypeOf(target) {
			touched.push('getPrototypeOf');
			return Reflect.getPrototypeOf(target);
		},
		setPrototypeOf() {
			touched.push('setPrototypeOf');
			return false;
		},
		isExtensible(target) {
			touched.push('isExtensible');
			return Reflect.isExtensible(target);
		},
		preventExtensions() {
			touched.push('preventExtensions');
			return false;
		}
	};
	return new Proxy({}, handler) as HTMLElement;
}

function stillRows(subject: Moves): void {
	describe('the still moves', () => {
		it.each(Object.keys(EVERY_CALL) as MoveName[])(
			'%s attaches without touching its element and leaves nothing to clean up',
			(move) => {
				const touched: string[] = [];
				const onEnd = vi.fn();
				for (const attachment of EVERY_CALL[move](subject, onEnd)) {
					expect(attachment(refusingElement(touched))).toBeUndefined();
				}
				expect(touched).toEqual([]);
				expect(onEnd).not.toHaveBeenCalled();
			}
		);

		it('life never calls onEnd, even 10 lives later, with every timer run out', async () => {
			vi.useFakeTimers();
			try {
				const onEnd = vi.fn();
				for (let lives = 0; lives < 10; lives += 1) {
					subject.life(onEnd)(refusingElement([]));
					await vi.runAllTimersAsync();
					await vi.advanceTimersByTimeAsync(60_000);
				}
				expect(onEnd).not.toHaveBeenCalled();
			} finally {
				vi.useRealTimers();
			}
		});
	});
}

/** The events a pointer coming over an element fires, and going off it. */
const POINTER_OVER = ['pointerover', 'pointerenter', 'mouseover', 'mouseenter'] as const;
const POINTER_OFF = ['pointerout', 'pointerleave', 'mouseout', 'mouseleave'] as const;

function firePointer(element: HTMLElement, names: readonly string[]): void {
	for (const name of names) {
		const bubbles = !name.endsWith('enter') && !name.endsWith('leave');
		const event = name.startsWith('pointer')
			? new PointerEvent(name, { bubbles })
			: new MouseEvent(name, { bubbles });
		element.dispatchEvent(event);
	}
}

/** Presses `element` once by pointer, once with Enter and once with Space. */
function press(element: HTMLElement): void {
	element.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
	element.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }));
	for (const key of ['Enter', ' ']) {
		element.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
		element.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
	}
}

/** The DOM calls and setters that parse a string as HTML, by owner and name. */
function htmlSinks(): readonly (readonly [object, string])[] {
	return [
		[Element.prototype, 'innerHTML'],
		[Element.prototype, 'outerHTML'],
		[Element.prototype, 'insertAdjacentHTML'],
		[Element.prototype, 'setHTMLUnsafe'],
		[ShadowRoot.prototype, 'innerHTML'],
		[ShadowRoot.prototype, 'setHTMLUnsafe'],
		[Range.prototype, 'createContextualFragment'],
		[DOMParser.prototype, 'parseFromString'],
		[Document.prototype, 'write'],
		[Document.prototype, 'writeln'],
		[Document, 'parseHTMLUnsafe']
	];
}

/** Records each use of an HTML sink until `stop`, which puts every sink back. */
function watchHtmlSinks(): { readonly used: string[]; readonly stop: () => void } {
	const used: string[] = [];
	const restores = htmlSinks().flatMap(([owner, name]) => {
		const original = Object.getOwnPropertyDescriptor(owner, name);
		if (original === undefined) return [];
		const record = (): void => {
			used.push(name);
		};
		const value: unknown = original.value;
		const watched: PropertyDescriptor =
			typeof value === 'function'
				? {
						...original,
						value: function (this: unknown, ...args: unknown[]): unknown {
							record();
							return Reflect.apply(value, this, args);
						}
					}
				: {
						...original,
						set: function (this: unknown, next: unknown): void {
							record();
							original.set?.call(this, next);
						}
					};
		Object.defineProperty(owner, name, watched);
		return [() => Object.defineProperty(owner, name, original)];
	});
	return {
		used,
		stop: () => {
			for (const restore of restores) restore();
		}
	};
}

/** Collects every element added anywhere in the document, with its descendants. */
function watchAdded(): { readonly take: () => readonly Element[]; readonly stop: () => void } {
	const delivered: MutationRecord[] = [];
	const observer = new MutationObserver((records) => {
		delivered.push(...records);
	});
	observer.observe(document.documentElement, { childList: true, subtree: true });
	return {
		take: () =>
			[...delivered.splice(0), ...observer.takeRecords()]
				.flatMap((record) => [...record.addedNodes])
				.filter((node) => node instanceof Element)
				.flatMap((element) => [element, ...element.querySelectorAll('*')]),
		stop: () => {
			observer.disconnect();
		}
	};
}

/** The properties every animation running on `element` writes. */
function animatedProperties(element: Element): readonly string[] {
	const timing = new Set(['offset', 'computedOffset', 'easing', 'composite']);
	return element
		.getAnimations()
		.flatMap((animation) =>
			animation.effect instanceof KeyframeEffect ? animation.effect.getKeyframes() : []
		)
		.flatMap((keyframe) => Object.keys(keyframe).filter((key) => !timing.has(key)));
}

function adapterRows(subject: Moves, adapter: ContractAdapter): void {
	const { mode, lifeMs, settleMs } = adapter;
	const cleanups: (() => void)[] = [];

	afterEach(() => {
		for (const cleanup of cleanups.splice(0)) cleanup();
	});

	/** A fresh container for this row, removed after it. */
	function container(): HTMLElement {
		const mounted = adapter.mount();
		cleanups.push(() => {
			mounted.remove();
		});
		return mounted;
	}

	function child<Tag extends keyof HTMLElementTagNameMap>(
		parent: HTMLElement,
		tag: Tag
	): HTMLElementTagNameMap[Tag] {
		return parent.appendChild(document.createElement(tag));
	}

	/** Attaches as Svelte does and hands back the teardown; it runs once, at most. */
	function attach(element: HTMLElement, attachment: Attachment<HTMLElement>): () => void {
		const teardown = attachment(element);
		let done = false;
		const once = (): void => {
			if (done) return;
			done = true;
			if (typeof teardown === 'function') teardown();
		};
		cleanups.unshift(once);
		return once;
	}

	describe(`the adapter's moves, ${mode}`, () => {
		it('life calls onEnd once, and never before the life is over', async () => {
			const onEnd = vi.fn();
			attach(child(container(), 'div'), subject.life(onEnd));
			await adapter.advance(lifeMs - 1);
			expect(onEnd).not.toHaveBeenCalled();
			await adapter.advance(settleMs + 1);
			expect(onEnd).toHaveBeenCalledOnce();
			await adapter.advance(lifeMs + settleMs);
			expect(onEnd).toHaveBeenCalledOnce();
		});

		it('life never calls onEnd once its element is torn down and removed', async () => {
			const onEnd = vi.fn();
			const element = child(container(), 'div');
			const teardown = attach(element, subject.life(onEnd));
			await adapter.advance(Math.floor(lifeMs / 2));
			teardown();
			element.remove();
			await adapter.advance(lifeMs + settleMs);
			expect(onEnd).not.toHaveBeenCalled();
		});

		it("life's wait pauses while the pointer is over it", async () => {
			const onEnd = vi.fn();
			const element = child(container(), 'div');
			attach(element, subject.life(onEnd));
			firePointer(element, POINTER_OVER);
			await adapter.advance(lifeMs + settleMs);
			expect(onEnd).not.toHaveBeenCalled();
			firePointer(element, POINTER_OFF);
			await adapter.advance(lifeMs + settleMs);
			expect(onEnd).toHaveBeenCalledOnce();
		});

		it("life's wait pauses while focus is inside it", async () => {
			const onEnd = vi.fn();
			const element = child(container(), 'div');
			const inside = child(element, 'button');
			attach(element, subject.life(onEnd));
			inside.focus();
			expect(document.activeElement).toBe(inside);
			await adapter.advance(lifeMs + settleMs);
			expect(onEnd).not.toHaveBeenCalled();
			inside.blur();
			await adapter.advance(lifeMs + settleMs);
			expect(onEnd).toHaveBeenCalledOnce();
		});

		if (mode === 'still') {
			it('life still calls onEnd under Still', async () => {
				const onEnd = vi.fn();
				attach(child(container(), 'div'), subject.life(onEnd));
				await adapter.advance(lifeMs + settleMs);
				expect(onEnd).toHaveBeenCalledOnce();
			});
		}

		const exits: readonly (readonly [string, Attachment<HTMLElement>])[] = [
			...ORIGINS.map((origin) => [`note('${origin}')`, subject.note(origin)] as const),
			['dim()', subject.dim()]
		];

		it.each(exits)('%s leaves through animations that end', async (_, attachment) => {
			const element = child(container(), 'div');
			element.dataset.state = 'open';
			attach(element, attachment);
			await adapter.advance(settleMs);
			element.dataset.state = 'closed';
			await adapter.advance(0);
			const endTimes = element
				.getAnimations()
				.map((animation) => Number(animation.effect?.getComputedTiming().endTime ?? 0));
			expect(endTimes.filter((endTime) => !Number.isFinite(endTime))).toEqual([]);
		});

		it('burst throws hidden pieces that take no pointer, built with no HTML string, and clears them', async () => {
			const origin = child(container(), 'div');
			const sinks = watchHtmlSinks();
			const added = watchAdded();
			try {
				attach(origin, subject.burst());
				await adapter.advance(0);
				const pieces = [...added.take()];
				for (const piece of pieces) {
					expect(piece.closest('[aria-hidden="true"]')).not.toBeNull();
					expect(getComputedStyle(piece).pointerEvents).toBe('none');
				}
				await adapter.advance(settleMs);
				pieces.push(...added.take());
				expect(pieces.filter((piece) => piece.isConnected)).toEqual([]);
				expect(sinks.used).toEqual([]);
			} finally {
				added.stop();
				sinks.stop();
			}
		});

		it("burst's pieces go when it is torn down", async () => {
			const origin = child(container(), 'div');
			const added = watchAdded();
			try {
				const teardown = attach(origin, subject.burst());
				await adapter.advance(0);
				const pieces = added.take();
				teardown();
				expect(pieces.filter((piece) => piece.isConnected)).toEqual([]);
			} finally {
				added.stop();
			}
		});

		it('squish never writes transform', async () => {
			const button = child(container(), 'button');
			button.style.transform = 'translate(-50%, -50%)';
			const written = button.style.transform;
			const drawn = getComputedStyle(button).transform;
			attach(button, subject.squish());
			const checkpoints = [0, Math.floor(settleMs / 2), settleMs];
			for (const wait of checkpoints) {
				press(button);
				await adapter.advance(wait);
				expect(button.style.transform).toBe(written);
				expect(getComputedStyle(button).transform).toBe(drawn);
				expect(animatedProperties(button)).not.toContain('transform');
			}
		});

		if (mode === 'reduced' || mode === 'still') {
			it.each(LOOP_KINDS)('loop(%s) leaves no running animation', async (kind) => {
				const element = child(container(), 'div');
				attach(element, subject.loop(kind));
				await adapter.advance(settleMs);
				const running = element
					.getAnimations({ subtree: true })
					.filter((animation) => animation.playState === 'running');
				expect(running).toEqual([]);
			});
		}
	});
}

/** Registers the moves contract's rows against `subject`. */
export function movesContract(subject: Moves, { adapter }: ContractOptions): void {
	if (adapter === undefined) {
		stillRows(subject);
		return;
	}
	adapterRows(subject, adapter);
}
