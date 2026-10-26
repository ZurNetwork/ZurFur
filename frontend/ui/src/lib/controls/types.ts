/** The item shapes the controls take: plain, readonly data, generic over the id where a control hands one back. */
import type { IconName } from '../icons/icons.ts';

/** A Button's look: primary is the view's one main action, destructive deletes or can't be undone. */
export type ButtonVariant = 'primary' | 'secondary' | 'quiet' | 'destructive';

/** A Button's height: `sm` for rows and cards. */
export type ButtonSize = 'md' | 'sm';

/** An IconButton's look. */
export type IconButtonVariant = 'quiet' | 'secondary';

/** An IconButton's size. */
export type IconButtonSize = 'md' | 'sm';

/** One tab: its label, an optional icon and an optional muted count, already written as text. */
export interface TabItem<Id extends string> {
	readonly id: Id;
	readonly label: string;
	readonly icon?: IconName;
	readonly count?: string;
}

/** One choice in a pill switch; `prefix` is the muted type before the label, such as `batch:`. */
export interface PillOption<Id extends string> {
	readonly id: Id;
	readonly label: string;
	readonly icon?: IconName;
	readonly prefix?: string;
	readonly disabled?: boolean;
}

/** What a field's frame hands the control inside it, so the control can point at its label, hint and error. */
export interface FieldIds {
	/** The id of the element that holds the label's words. */
	readonly labelId: string;
	/** The ids of the hint and the error, whichever are shown, ready for `aria-describedby`. */
	readonly describedBy: string | undefined;
	/** `'true'` while the field has an error, otherwise absent. */
	readonly invalid: 'true' | undefined;
}

/** One choice in a Select. */
export interface SelectOption<V extends string> {
	readonly value: V;
	readonly label: string;
}

/** One row in a menu; `danger` is for a row that deletes or can't be undone. */
export interface MenuItem<Id extends string> {
	readonly id: Id;
	readonly label: string;
	readonly icon?: IconName;
	readonly disabled?: boolean;
	readonly tone?: 'danger';
}

/** What opens a menu: a button with words, or an icon alone whose label is its accessible name. */
export type MenuTrigger =
	| {
			readonly kind: 'button';
			readonly label: string;
			readonly icon?: IconName;
			readonly variant?: ButtonVariant;
	  }
	| {
			readonly kind: 'icon';
			readonly label: string;
			readonly icon: IconName;
	  };

/** The words a date field speaks for its segments, and for a segment with nothing in it. */
export interface DateFieldWords {
	readonly day: string;
	readonly month: string;
	readonly year: string;
	readonly empty: string;
}
