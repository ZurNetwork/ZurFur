/** An easing as the token writes it, read for drawing. */
export type EasingShape =
	| {
			readonly kind: 'bezier';
			readonly x1: number;
			readonly y1: number;
			readonly x2: number;
			readonly y2: number;
	  }
	| { readonly kind: 'linear' }
	| { readonly kind: 'unplottable' };

const NUMBER = '(-?(?:\\d+\\.?\\d*|\\.\\d+))';
const BEZIER = new RegExp(
	`^cubic-bezier\\(\\s*${[NUMBER, NUMBER, NUMBER, NUMBER].join('\\s*,\\s*')}\\s*\\)$`
);

/** Reads `linear` or `cubic-bezier(a, b, c, d)`; any other text is unplottable. */
export function parseEasing(text: string): EasingShape {
	const trimmed = text.trim();
	if (trimmed === 'linear') return { kind: 'linear' };
	const match = BEZIER.exec(trimmed);
	const [x1, y1, x2, y2] = (match?.slice(1) ?? []).map(Number);
	if (x1 === undefined || y1 === undefined || x2 === undefined || y2 === undefined) {
		return { kind: 'unplottable' };
	}
	if (![x1, y1, x2, y2].every(Number.isFinite)) return { kind: 'unplottable' };
	return { kind: 'bezier', x1, y1, x2, y2 };
}

/** What an inline SVG needs to draw an easing: its box, the curve and the straight reference. */
export interface CurvePlot {
	readonly viewBox: string;
	readonly curve: string;
	readonly diagonal: string;
}

const SIZE = 100;
const MARGIN = 6;

function figure(value: number): string {
	return String(Number(value.toFixed(2)));
}

/** The plot of an easing, or undefined when it cannot be drawn. Time runs right, progress up. */
export function plotOf(shape: EasingShape): CurvePlot | undefined {
	const diagonal = `M0 ${figure(SIZE)} L${figure(SIZE)} 0`;
	switch (shape.kind) {
		case 'unplottable':
			return undefined;
		case 'linear':
			return {
				viewBox: `${figure(-MARGIN)} ${figure(-MARGIN)} ${figure(SIZE + 2 * MARGIN)} ${figure(SIZE + 2 * MARGIN)}`,
				curve: diagonal,
				diagonal
			};
		case 'bezier': {
			const low = Math.min(0, shape.y1, shape.y2);
			const high = Math.max(1, shape.y1, shape.y2);
			const top = SIZE - high * SIZE - MARGIN;
			const height = (high - low) * SIZE + 2 * MARGIN;
			const control = (x: number, y: number): string =>
				`${figure(x * SIZE)} ${figure(SIZE - y * SIZE)}`;
			return {
				viewBox: `${figure(-MARGIN)} ${figure(top)} ${figure(SIZE + 2 * MARGIN)} ${figure(height)}`,
				curve: `M0 ${figure(SIZE)} C${control(shape.x1, shape.y1)} ${control(shape.x2, shape.y2)} ${figure(SIZE)} 0`,
				diagonal
			};
		}
	}
}
