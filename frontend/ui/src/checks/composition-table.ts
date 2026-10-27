/**
 * The lego table: what kind each exported component is, which files may
 * render each owned element, which `zf-` families each file may write, which
 * bits-ui names each file may import, and the named exceptions.
 * `composition.spec.ts` reads it over every file under `src/`. A row added
 * here widens what a file may draw, so a change that adds one gets a
 * security review.
 */

/** A simple component draws one piece itself; a composite only lays out library components. */
export type Kind = 'simple' | 'composite';

/** A class family: the class itself, plus every class that starts with it and `__` or `--`. */
export type Family = `zf-${string}`;

/** A file, by its path relative to the package (`src/...`, `/`-separated). */
export type FilePath = `src/${string}`;

/** A table row's file, and why it is there. */
export interface Row {
	readonly file: FilePath;
	readonly reason: string;
}

/** The kind of every component the public entry exports, by its exported name. */
export const KINDS: Readonly<Record<string, Kind>> = {
	Icon: 'simple',
	Button: 'simple',
	IconButton: 'simple',
	Toggle: 'simple',
	Checkbox: 'simple',
	TextInput: 'simple',
	Select: 'simple',
	Tabs: 'simple',
	PillSwitch: 'simple',
	DateField: 'simple',
	Menu: 'simple',
	ToastStack: 'simple',
	Card: 'simple',
	Badge: 'simple',
	Spinner: 'simple',
	Skeleton: 'simple',
	VisibilityMarker: 'simple',
	StatusPill: 'simple'
};

/** The glyphs a simple component may hold, by exported name. Spinner's glyph form is what Button's busy state holds. */
export const GLYPHS: readonly string[] = ['Icon', 'Spinner'];

/** The library's internal pieces: simple, not exported, and usable by any library component. */
export const INTERNAL: readonly Row[] = [
	{
		file: 'src/lib/controls/FieldFrame.svelte',
		reason: 'the label, hint and error laid out around a field'
	},
	{
		file: 'src/lib/controls/Tooltip.svelte',
		reason: "the visual-only tip IconButton's label shows in"
	}
];

/** The elements a file may render only when it is listed for them. */
export const OWNED_ELEMENTS = [
	'button',
	'a',
	'input',
	'select',
	'textarea',
	'img',
	'video',
	'audio',
	'iframe',
	'image'
] as const;

/** An owned element; `image` is SVG's (an HTML `<image>` reads as `img`). */
export type OwnedElement = (typeof OWNED_ELEMENTS)[number];

/** The files that may render each owned element. */
export const OWNERS: Readonly<Record<OwnedElement, readonly Row[]>> = {
	button: [
		{ file: 'src/lib/controls/Button.svelte', reason: 'the button itself' },
		{ file: 'src/lib/controls/IconButton.svelte', reason: 'the button itself' },
		{ file: 'src/lib/controls/Toggle.svelte', reason: "the switch's track" },
		{ file: 'src/lib/controls/Tabs.svelte', reason: 'each tab' },
		{ file: 'src/lib/controls/PillSwitch.svelte', reason: 'each pill' },
		{ file: 'src/gallery/MoveSheet.svelte', reason: "the old gallery's Play again on a move" },
		{
			file: 'src/stories/tokens/PaperPage.svelte',
			reason:
				'the Paper token page shows the paper classes on bare buttons; a token page, not a component'
		}
	],
	a: [
		{ file: 'src/lib/controls/Button.svelte', reason: 'the link form, given a LinkHref' },
		{ file: 'src/gallery/GalleryNav.svelte', reason: "the old gallery's nav links" }
	],
	input: [
		{ file: 'src/lib/controls/TextInput.svelte', reason: 'the single-line field' },
		{ file: 'src/lib/controls/Checkbox.svelte', reason: 'the native checkbox' },
		{ file: 'src/lib/controls/DateField.svelte', reason: 'the hidden input that posts the day' },
		{ file: 'src/gallery/SkinSwitch.svelte', reason: "the old gallery's Skin switch" }
	],
	select: [{ file: 'src/lib/controls/Select.svelte', reason: 'the native select' }],
	textarea: [{ file: 'src/lib/controls/TextInput.svelte', reason: 'the multiline field' }],
	img: [],
	video: [],
	audio: [],
	iframe: [],
	image: []
};

/** The files that may render a `<svelte:element>` whose `this` is not written out. */
export const DYNAMIC_ELEMENT: readonly Row[] = [
	{
		file: 'src/lib/surfaces/Card.svelte',
		reason: 'the card becomes a div, section, article or li, chosen from a closed map'
	}
];

/** The elements no file under `src/lib/` or `src/stories/` renders: they load, base or style the page. */
export const PAGE_ELEMENTS: readonly string[] = ['link', 'base', 'svelte:head', 'style'];

/** The folders `PAGE_ELEMENTS` are kept out of. */
export const PAGE_ELEMENT_FOLDERS: readonly string[] = ['src/lib/', 'src/stories/'];

/** The families each family stylesheet and each library file may write. */
export const FAMILIES: Readonly<Record<FilePath, readonly Family[]>> = {
	'src/lib/styles/controls/button.css': ['zf-btn', 'zf-iconbtn'],
	'src/lib/styles/controls/field.css': ['zf-field', 'zf-input', 'zf-select', 'zf-date'],
	'src/lib/styles/controls/check.css': ['zf-check'],
	'src/lib/styles/controls/toggle.css': ['zf-toggle'],
	'src/lib/styles/controls/tabs.css': ['zf-tabs', 'zf-tab', 'zf-tabview', 'zf-seg'],
	'src/lib/styles/controls/menu.css': ['zf-menu'],
	'src/lib/styles/controls/tip.css': ['zf-tip'],
	'src/lib/styles/surfaces/card.css': ['zf-card'],
	'src/lib/styles/surfaces/dialog.css': ['zf-backdrop', 'zf-dialog'],
	'src/lib/styles/surfaces/toast.css': ['zf-toast', 'zf-toast-stack'],
	'src/lib/styles/surfaces/message.css': ['zf-inline', 'zf-empty'],
	'src/lib/styles/surfaces/loading.css': [
		'zf-spinner',
		'zf-spinner-wrap',
		'zf-skeleton',
		'zf-skeleton-lines'
	],
	'src/lib/styles/surfaces/label.css': [
		'zf-badge',
		'zf-status',
		'zf-pill',
		'zf-vis',
		'zf-avatar',
		'zf-avatar-wrap',
		'zf-handle'
	],
	'src/lib/styles/surfaces/mature.css': ['zf-mature'],
	'src/lib/styles/surfaces/pin.css': ['zf-pin'],
	'src/lib/icons/Icon.svelte': ['zf-icon'],
	'src/lib/controls/Button.svelte': ['zf-btn'],
	'src/lib/controls/IconButton.svelte': ['zf-iconbtn'],
	'src/lib/controls/FieldFrame.svelte': ['zf-field'],
	'src/lib/controls/TextInput.svelte': ['zf-input'],
	'src/lib/controls/Select.svelte': ['zf-select'],
	'src/lib/controls/DateField.svelte': ['zf-date'],
	'src/lib/controls/Checkbox.svelte': ['zf-check'],
	'src/lib/controls/Toggle.svelte': ['zf-toggle'],
	'src/lib/controls/Tabs.svelte': ['zf-tabs', 'zf-tab', 'zf-tabview'],
	'src/lib/controls/PillSwitch.svelte': ['zf-seg'],
	'src/lib/controls/Menu.svelte': ['zf-menu'],
	'src/lib/controls/Tooltip.svelte': ['zf-tip'],
	'src/lib/surfaces/ToastStack.svelte': ['zf-toast-stack'],
	'src/lib/surfaces/Card.svelte': ['zf-card'],
	'src/lib/surfaces/Badge.svelte': ['zf-badge'],
	'src/lib/surfaces/Spinner.svelte': ['zf-spinner', 'zf-spinner-wrap'],
	'src/lib/surfaces/Skeleton.svelte': ['zf-skeleton', 'zf-skeleton-lines'],
	'src/lib/surfaces/VisibilityMarker.svelte': ['zf-vis'],
	'src/lib/surfaces/StatusPill.svelte': ['zf-status', 'zf-pill']
};

/** The families any `.svelte` file under `src/lib/` may write: `zf-sr` keeps words for screen readers only. */
export const SHARED_FAMILIES: readonly Family[] = ['zf-sr'];

/** A named reach past the rules: owned elements or families a file uses that are not its own. */
export interface Exception extends Row {
	readonly elements: readonly OwnedElement[];
	readonly families: readonly Family[];
}

/** The named exceptions. They only shrink: `composition.spec.ts` holds the known list. */
export const EXCEPTIONS: readonly Exception[] = [
	{
		file: 'src/lib/controls/Menu.svelte',
		elements: ['button'],
		families: ['zf-btn', 'zf-iconbtn'],
		reason:
			"bits-ui's DropdownMenu.Trigger must own the trigger element, so Menu draws it as a button"
	},
	{
		file: 'src/lib/controls/DateField.svelte',
		elements: [],
		families: ['zf-input'],
		reason: "the date field wears the text field's frame, so it looks like every other field"
	},
	{
		file: 'src/lib/styles/controls/menu.css',
		elements: [],
		families: ['zf-icon'],
		reason: "a menu row's icon takes the row's colour, the danger row's included"
	}
];

/** The names each file may import from bits-ui. No row may list Avatar or Button. */
export const BITS_UI: Readonly<Record<FilePath, readonly string[]>> = {
	'src/lib/controls/Toggle.svelte': ['Switch'],
	'src/lib/controls/Tabs.svelte': ['Tabs'],
	'src/lib/controls/PillSwitch.svelte': ['RadioGroup'],
	'src/lib/controls/Menu.svelte': ['DropdownMenu'],
	'src/lib/controls/DateField.svelte': ['DateField'],
	'src/lib/controls/Tooltip.svelte': ['Tooltip']
};

/** A bits-ui part, by its namespace and its name in it. */
export interface BitsPart {
	readonly namespace: string;
	readonly part: string;
}

/** The bits-ui parts that render a link or an image themselves: no file renders them, by any name. */
export const UNOWNED_BITS_PARTS: readonly BitsPart[] = [
	{ namespace: 'Avatar', part: 'Image' },
	{ namespace: 'Button', part: 'Root' },
	{ namespace: 'LinkPreview', part: 'Trigger' },
	{ namespace: 'NavigationMenu', part: 'Link' },
	{ namespace: 'Toolbar', part: 'Link' },
	{ namespace: 'Command', part: 'LinkItem' }
];

/** The bits-ui namespaces no row may list: their parts draw a link or an image. */
export const UNLISTED_BITS_NAMESPACES: readonly string[] = ['Avatar', 'Button'];

/** The whole table, as the check reads it. */
export interface CompositionTable {
	readonly kinds: Readonly<Record<string, Kind>>;
	readonly glyphs: readonly string[];
	readonly internal: readonly Row[];
	readonly owners: Readonly<Record<OwnedElement, readonly Row[]>>;
	readonly dynamicElement: readonly Row[];
	readonly families: Readonly<Record<FilePath, readonly Family[]>>;
	readonly sharedFamilies: readonly Family[];
	readonly exceptions: readonly Exception[];
	readonly bitsUi: Readonly<Record<FilePath, readonly string[]>>;
}

export const TABLE: CompositionTable = {
	kinds: KINDS,
	glyphs: GLYPHS,
	internal: INTERNAL,
	owners: OWNERS,
	dynamicElement: DYNAMIC_ELEMENT,
	families: FAMILIES,
	sharedFamilies: SHARED_FAMILIES,
	exceptions: EXCEPTIONS,
	bitsUi: BITS_UI
};
