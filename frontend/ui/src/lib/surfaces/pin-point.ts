/** A point on a picture, and the one way to mint it. */

declare const pinPointBrand: unique symbol;

/** A point on a picture, as fractions of its width and height: each a finite number from 0 to 1. */
export type PinPoint = Readonly<{ x: number; y: number }> & { readonly [pinPointBrand]: true };

/** Whether `value` is a finite number from 0 to 1, both ends included. */
function isFraction(value: unknown): value is number {
	return typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 1;
}

/** Whether both of the point's numbers are fractions. */
function isPinPoint(point: Readonly<{ x: unknown; y: unknown }>): point is PinPoint {
	return isFraction(point.x) && isFraction(point.y);
}

/** The value, with `-0` turned into `0`. */
function withoutNegativeZero(value: unknown): unknown {
	return Object.is(value, -0) ? 0 : value;
}

/** A frozen point from the two values read once, or nothing when either is not a fraction. */
function mint(x: unknown, y: unknown): PinPoint | undefined {
	const point = Object.freeze({ x: withoutNegativeZero(x), y: withoutNegativeZero(y) });
	return isPinPoint(point) ? point : undefined;
}

/** The point at `x` across and `y` down, each from 0 to 1; nothing when either is out of range. */
export function pinPointOf(x: number, y: number): PinPoint | undefined {
	return mint(x, y);
}

/** Any value checked again as a point: a fresh point from its `x` and `y`, or nothing. */
export function pinPointFrom(value: unknown): PinPoint | undefined {
	if (typeof value !== 'object' || !value || !('x' in value) || !('y' in value)) return undefined;
	return mint(value.x, value.y);
}
