/** The args each control's stories hand the story template: the control's own props. */
import type { ComponentProps } from 'svelte';
import type {
	Checkbox,
	DateField,
	Menu,
	PillSwitch,
	ScrollingTabs,
	Select,
	Tabs,
	TextInput,
	Toggle
} from '@zurfur/ui';

export type CheckboxArgs = ComponentProps<typeof Checkbox>;
export type ToggleArgs = ComponentProps<typeof Toggle>;
export type PillSwitchArgs = ComponentProps<typeof PillSwitch>;
export type TextInputArgs = ComponentProps<typeof TextInput>;
export type SelectArgs = ComponentProps<typeof Select>;
export type TabsArgs = ComponentProps<typeof Tabs>;
export type ScrollingTabsArgs = ComponentProps<typeof ScrollingTabs>;
export type MenuArgs = ComponentProps<typeof Menu>;
export type DateFieldArgs = ComponentProps<typeof DateField>;
