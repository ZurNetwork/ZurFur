/** Every class `paper-moves.css` defines: the moves, applied only through the motion adapter. */
export const PAPER_MOVES = [
	'paperin',
	'paperlift',
	'paperpop',
	'paperup',
	'paperland',
	'paperlife',
	'paperlife-shadow',
	'paperlife-self',
	'deskdim',
	'deskclear',
	'hop',
	'pagein',
	'pageout',
	'pageunder'
] as const;

/** One class from `paper-moves.css`. */
export type PaperMove = (typeof PAPER_MOVES)[number];
