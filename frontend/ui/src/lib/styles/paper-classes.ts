/** Every class `paper.css` defines: the desk, the sheets and the small things laid on them. */
export const PAPER_CLASSES = [
	'paper',
	'pile',
	'peek',
	'pk1',
	'pk2',
	'pk3',
	'lift',
	'lift-art',
	'lifts',
	'obj',
	'placed',
	'glide',
	'glidex',
	'seg',
	'glidebox',
	'piece',
	'pick',
	'is-locked',
	'is-on',
	'slip'
] as const;

/** One class from `paper.css`. */
export type PaperClass = (typeof PAPER_CLASSES)[number];
