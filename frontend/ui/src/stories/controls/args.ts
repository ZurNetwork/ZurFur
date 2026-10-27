/** The args each control's stories hand the story template: the control's own props. */
import type { ComponentProps } from 'svelte';
import type { DateField, Menu, Select, Tabs, TextInput } from '@zurfur/ui';

export type TextInputArgs = ComponentProps<typeof TextInput>;
export type SelectArgs = ComponentProps<typeof Select>;
export type TabsArgs = ComponentProps<typeof Tabs>;
export type MenuArgs = ComponentProps<typeof Menu>;
export type DateFieldArgs = ComponentProps<typeof DateField>;
