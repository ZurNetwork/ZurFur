> **Superseded** by `20261008-design-language.md` (2026-10-08). Use that one.

# Zurfur — design language brief

**What Zurfur is.** A web app where artists and the people who commission them run commissions together. That covers characters and sonas, ref sheets, YCHs, fursuits and stories, and the community it grows from is the furry art community. It is built on the AT Protocol (Bluesky's network), so people sign in with their Bluesky identity and own it. Handles like `@zuri.bsky.social` appear everywhere.

**What I need now.** A design language: tokens and a small set of core components, not full screens. Start by proposing **2–3 distinct directions**. Give each one a name, three mood words, a palette, a type pairing, and one sample screen: the main layout with a commission open. I'll pick a direction, and then you build its full token set and components.

**My taste:** _(fill in: mood words, apps or sites you like, anything to avoid)_

## Principles the look must serve
- **Calm, never shaming.** The product's top rule is "choose the non-toxic path": people's wellbeing beats mechanical correctness.
  - No red "LATE" badges, alarm colours or warning banners about people's states. A missed deadline reads as a quiet fact, not a mark of shame.
  - Status is information, not judgment.
  - No leaderboards, streaks or engagement bait.
- **The art is the star.** The interface steps back. Neutral chrome, so artwork colours never fight the UI.
- **Explicit, never sneaky.** Nothing happens on a user's behalf. Every action is a clearly labelled, deliberate control. Destructive actions look different from everyday ones.
- **We work with the user.** The tone is friendly and plain-spoken: a studio tool made by people in the community, not a corporate dashboard.

## The structure it has to fit
- **Explorer layout.** A folder tree on the left, the open item on the right, and its path along the top, like Obsidian or VS Code. On phones the tree becomes a slide-out drawer. Everything a user owns is a node in that tree:
  - accounts, commissions, characters, their profile;
  - inside a commission: attachments, products, slots, a changelog and participants.
- **Dense but readable.** People scan long trees and timelines. A comfortable default density, with clear hierarchy.
- **Commission status.** A commission has a lifecycle plus a two-part status (for example "in progress" or "waiting on client"). It needs a calm, distinguishable status palette. Never red for a person's state.
- **Visibility.** Every item is **Private**, **Listed** or **Public**, so it needs three consistent, recognizable markers.
- **Maturity.** Artwork carries a rating: Safe, Suggestive, Nudity, Adult, plus a separate Graphic flag. Mature art is **blurred by default for everyone, never hidden**, with a click-through ("I'm 18+"). The blur cover should feel respectful and neutral: no warning-sign styling, no shame.
- **Comments as pins.** Reviewers drop pins on an image, and each pin opens a thread. Pins must stay legible on any artwork, light or dark.
- **The changelog** is an append-only timeline of everything that happened in a commission. It must be easy to scan.
- **Hidden configuration files** ("dotfiles") can be shown with a toggle, styled as quieter than normal items.

## Must-haves
- **Light and dark themes,** both first-class.
- **Accessible:** WCAG 2.2 AA contrast, visible focus rings, a fully keyboard-usable tree, and respect for reduced-motion settings.
- **Responsive** from 320 px phones to wide desktops.
- **Tokens as CSS custom properties,** with semantic names that components use, not raw palette names. For example `--color-surface`, `--color-text-muted`, `--color-status-active`, `--color-visibility-public`, `--space-3`, `--radius-md`. Include:
  - colour (backgrounds, surfaces, text, borders, accent, focus, status, visibility);
  - a type scale;
  - spacing;
  - radii;
  - elevation;
  - motion durations.
- **Open-licensed fonts** (Google Fonts or OFL) and **one consistent open-source icon set.**
- **Core components:** button (primary, secondary, quiet, destructive), icon button, text input, select, checkbox, toggle, tree item (folder, file, link, mounted), path bar, tabs, card, badge or chip, avatar with handle, dialog, toast, empty state, loading skeleton, the blurred-media cover, and a status pill.
- **Not needed yet:** a logo, marketing pages, or full screens beyond the samples.
