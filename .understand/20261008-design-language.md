# Zurfur — design language

**Zurfur** is a web app where artists and the people who commission them run commissions together. That covers characters and sonas, ref sheets, YCHs, fursuits and stories, and it grows from the furry art community. It is built on the AT Protocol (Bluesky's network): people sign in with their Bluesky identity and own it, so handles like `@zuri.bsky.social` appear everywhere.

Build a **design system** from this: tokens and core components, not full screens.

## 1. Personality
- **A cosy studio, not a corporate dashboard.** Warm, friendly and plain-spoken, made by people in the community.
- **Calm, never shaming.** The product's top rule is "choose the non-toxic path": people's wellbeing beats mechanical correctness.
- **The art is the star.** The interface steps back so artwork carries the colour.
- **Explicit, never sneaky.** Nothing happens on a user's behalf. Every action is a clearly labelled, deliberate control.

## 2. Colour
- **Dark mode is the default.** Light mode is a complete alternate theme, not an afterthought. Design dark first.
- **The palette comes from my sona's colours:** _(fill in: name each colour and give its hex)_
  - Take the **accent** from the sona's most distinctive colour. Use it sparingly: primary actions, focus, the selected tree item, links.
  - Take the **neutrals** (backgrounds, surfaces, borders, text) from the sona's base colour, tinted only slightly, so the chrome stays quiet next to artwork of any palette.
  - Any remaining sona colours become **secondary accents** (tags, avatars, the visibility markers), never large fills.
- **Status colours** are calm and tell states apart without judging them. **Never red for a person's state:** a missed deadline reads as a quiet fact, never a red "LATE" badge. Red, or the strongest warning hue, is kept only for destructive actions ("Delete") and system errors.
- **Visibility markers:** three consistent, recognizable treatments for **Private**, **Listed** and **Public**. Each pairs an icon with a colour and never relies on colour alone.
- **Contrast:** at least WCAG 2.2 AA in both themes, for text, icons and focus rings.

## 3. Type
- One open-licensed **UI sans** (Google Fonts or OFL), readable at small sizes in dense trees.
- One **monospace** for identifiers: handles, DIDs and file paths.
- A modest type scale. Hierarchy comes from weight and spacing more than from size.

## 4. Space, shape, depth
- **Dense but readable.** People scan long folder trees and timelines. A comfortable default density, on a 4 px base spacing scale.
- **Soft, friendly radii,** never pill-everything.
- **In dark mode, depth comes from surface steps and borders,** not heavy shadows.

## 5. Icons and motion
- **One open-source icon set,** with a consistent stroke and size.
- **Motion is short and purposeful:** a tree expanding, a drawer sliding, a toast arriving. Respect reduced-motion settings. Nothing bounces for attention.

## 6. Layout: the Explorer
- A **folder tree** on the left, the **open item** on the right, and its **path** along the top, like Obsidian or VS Code. On phones the tree becomes a slide-out drawer. Responsive from 320 px to wide desktops.
- Everything a user owns is a node in the tree. At the top: accounts, characters, commissions and posts. Inside a commission: attachments, products, slots, a changelog and participants. (Public profiles live outside anyone's tree.)
- Tree items come in three kinds: **folder**, **file** and **link** (a shortcut). Any of them can also carry a **mounted** marker, meaning it's shared into your tree from elsewhere. Each kind, and the marker, needs a quiet, distinct look.
- Some items are visible but **can't be opened** (you may know they exist, nothing more). They need a quiet locked look, not an error look.
- **Hidden configuration files** ("dotfiles") appear only behind a toggle, styled quieter than normal items.

## 7. Special treatments
- **Mature art.** Ratings are Safe, Suggestive, Nudity and Adult, plus a separate Graphic flag. Mature art is **blurred by default for everyone**, with a click-through ("I'm 18+"). Each viewer can choose to see it unblurred, or to hide it entirely. The blur cover is respectful and neutral: no warning signs, no shame.
- **Pins on artwork.** Reviewers drop pins on an image, and each pin opens a thread. Pins stay legible on any artwork, light or dark.
- **The changelog** is an append-only timeline of everything that happened in a commission. It must be easy to scan.
- **Commission status:** a lifecycle plus a two-part status, for example "in progress" or "waiting on client". Show it as a calm status pill.

## 8. Voice in the interface
- **Plain words:**
  - "Deadline passed", never "LATE" or "OVERDUE!";
  - "Waiting on client", never "Client unresponsive".
- **No sting:** declines are silent, and nothing names who said no.
- **No engagement bait:** no streaks, leaderboards, red counters or "you're missing out".
- **Buttons say what they do:** "Send invoice", not "Continue".

## 9. Tokens, as CSS custom properties
Use semantic names that components consume, never raw palette names: `--color-bg`, `--color-surface`, `--color-surface-raised`, `--color-border`, `--color-text`, `--color-text-muted`, `--color-accent`, `--color-on-accent`, `--color-hover`, `--color-selected`, `--color-backdrop`, `--color-focus`, `--color-danger`, `--color-status-*`, `--color-visibility-private|listed|public`, `--font-ui`, `--font-mono`, `--text-*`, `--space-*`, `--radius-*`, `--elevation-*`, `--duration-*`, and the layout sizes `--border-width`, `--focus-ring-width`, `--drawer-width`, `--pane-max-width`, `--dialog-width` (added 2026-10-08, from the first build). Define both themes, with dark as the default.

## 10. Core components
These names match the app's component library: Button (primary, secondary, quiet, destructive), IconButton, TextInput, Select, Checkbox, Toggle, TreeItem (folder, file, link; mounted marker; can't-open state; plus a list-row variant for the right pane), PathBar, Tabs, Card (a plain container), Badge, StatusPill, VisibilityMarker, Avatar (with handle), Dialog (with a drawer variant, the slide-out tree on phones), Toast, InlineMessage (info, error), EmptyState, Skeleton, MatureCover, Pin.

**Not needed yet:** a logo, marketing pages, or full screens.
