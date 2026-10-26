/** The gallery's nav groups, in the order the nav lists them. */
export const groupNames = [
	'Tokens and paper',
	'Controls',
	'Surfaces and feedback',
	'Navigation and patterns',
	'Motion and sound',
	'Heavy pieces',
	"Pieces for 0.3's screens"
] as const;

export type Group = (typeof groupNames)[number];

/** Whether the text names one of the gallery's groups. */
export function isGroup(text: string): text is Group {
	return groupNames.some((name) => name === text);
}
