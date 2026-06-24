# HoldRect Landing Page — Design Spec

**Date:** 2026-06-25
**Scope:** Single-page static landing page for GitHub Pages
**Style:** Anthropic design system (warm cream canvas, serif headlines, coral CTAs, dark navy surfaces)

---

## Files

| File | Purpose |
|------|---------|
| `index.html` | Page markup, structured in 6 semantic bands |
| `style.css` | All styles — layout, typography, colors, responsive |
| `assets/HoldRect.png` | Logo (already exists in repo) |
| `assets/HoldRect_show.gif` | Demo GIF (already exists in repo) |
| `assets/holdrect-demo-static.png` | Static screenshot fallback for reduced-motion (create from GIF) |

No build step. No JS framework. No dependencies beyond Google Fonts CDN.

> **Note:** Landing page copy is marketing-optimized and may differ from README wording. This spec is the source of truth for the landing page.

---

## Design System (Anthropic-derived)

### Colors

| Token | Hex | Use |
|-------|-----|-----|
| Canvas | `#faf9f5` | Page background, hero band, nav |
| Surface Card | `#efe9de` | Feature card backgrounds |
| Surface Dark | `#181715` | Install band, footer |
| Surface Dark Elevated | `#252320` | Code block inside dark band |
| Primary (Coral) | `#cc785c` | CTA buttons, coral CTA band |
| Primary Active | `#a9583e` | Button hover |
| Ink | `#141413` | Headlines, primary text |
| Body | `#3d3d3a` | Paragraph text |
| Muted | `#6c6a64` | Secondary text, captions |
| On Primary | `#ffffff` | Text on coral buttons |
| On Dark | `#faf9f5` | Text on dark surfaces |
| On Dark Soft | `#a09d96` | Secondary text on dark |
| Hairline | `#e6dfd8` | Borders, dividers |

### Typography

| Role | Font | Fallback | Size | Weight | Line Height | Letter Spacing |
|------|------|----------|------|--------|-------------|----------------|
| Display XL (h1) | Cormorant Garamond | Georgia, "Times New Roman", serif | 64px | 500 | 1.05 | -1.5px |
| Display LG (h2) | Cormorant Garamond | Georgia, "Times New Roman", serif | 48px | 500 | 1.1 | -1px |
| Display SM (h3) | Cormorant Garamond | Georgia, "Times New Roman", serif | 28px | 500 | 1.2 | -0.3px |
| Title (card h3) | Inter | sans-serif | 18px | 500 | 1.4 | 0 |
| Body | Inter | sans-serif | 16px | 400 | 1.55 | 0 |
| Body Sm | Inter | sans-serif | 14px | 400 | 1.55 | 0 |
| Caption | Inter | sans-serif | 12px | 500 | 1.4 | 1.5px |
| Code | JetBrains Mono | monospace | 14px | 400 | 1.6 | 0 |
| Button | Inter | sans-serif | 14px | 500 | 1.2 | 0 |

### Spacing

| Token | Value |
|-------|-------|
| xs | 8px |
| sm | 12px |
| md | 16px |
| lg | 24px |
| xl | 32px |
| xxl | 48px |
| section | 96px |

### Border Radius

| Token | Value | Use |
|-------|-------|-----|
| md | 8px | Buttons, inputs |
| lg | 12px | Cards |
| xl | 16px | Hero illustration card |

### Max Content Width

1200px, centered.

---

## Page Structure — 6 Bands

### 1. Navigation Bar

- **Background:** Canvas (`#faf9f5`)
- **Height:** 64px
- **Layout:** Flex, space-between
- **Left:** HoldRect logo (`HoldRect.png`, 28px height) + "HoldRect" wordmark in Inter 500
- **Right:** "GitHub" text link (ink color, nav-link style) + "Download" primary button (coral)
- **Sticky:** Yes, top-0 with subtle bottom hairline border
- **Mobile:** Logo left, Download button right. No hamburger menu (no JS).

### 2. Hero Band

- **Background:** Canvas (`#faf9f5`)
- **Padding:** `section` (96px) vertical
- **Layout:** 6/6 grid (text left, demo right), stacks on mobile
- **Left column:**
  - H1: "Highlight anything. Instantly." — Cormorant Garamond display-xl, ink color
  - Subtitle: "Hold Alt, drag a rectangle, done. A lightweight screen highlighter for recordings, presentations, and live demos — under 2 MB. Windows today, macOS & Linux coming soon." — Inter body-md, body color
  - Button row: Primary coral "Download for Windows" + text link "View on GitHub →"
- **Right column:**
  - GIF demo (`HoldRect_show.gif`) inside a `rounded-xl` dark card (`#181715`) with 24px padding — frames the demo like a product mockup
  - Max-width on GIF: 100%, height auto, centered in card
  - GIF note: the GIF has a light/opaque background, so add a subtle 1px `#252320` border on the GIF element to soften the hard edge against the dark card
- **Mobile:** Single column, text first then GIF below

### 3. Features Band

- **Background:** Surface Card (`#efe9de`) — one step darker than canvas
- **Padding:** `section` (96px) vertical
- **Heading:** "Why HoldRect?" — Cormorant Garamond display-lg, centered, ink color
- **Layout:** 2x2 grid on desktop, 1-up on mobile
- **Cards:**
  - **Card 1 — Zero-Mode Interaction:** Icon ⚡, "No toolbar, no hotkey sequence. Hold Alt and drag — that's the entire interface." — Inter body-md
  - **Card 2 — Rainbow Border:** Icon 🌈, "Gradient flows along the rectangle perimeter. Unique to HoldRect. Your audience sees exactly what you mean." — Inter body-md
  - **Card 3 — Pin & Spotlight:** Icon 📌, "Press 1 to pin the rectangle on screen. Press 2 to dim everything outside. Toggle anytime." — Inter body-md
  - **Card 4 — Ultra-Lightweight:** Icon 🪶, "Under 2 MB. Rust native binary with no runtime, no Electron, no installer bloat. You'll forget it's running." — Inter body-md
- **Card style:** Background canvas (`#faf9f5`) — deliberately one step lighter than the band background (`#efe9de`) to create a raised/inset contrast, matching Anthropic's cream-on-card surface rhythm. Rounded-lg (12px), padding xl (32px), 1px hairline border

### 4. Install Band (Dark)

- **Background:** Surface Dark (`#181715`)
- **Padding:** `section` (96px) vertical
- **Heading:** "Get started in seconds" — Cormorant Garamond display-lg, on-dark color
- **Content:** Dark elevated card (`#252320`, rounded-lg, padding-lg) containing:
  - Code block: PowerShell install command in JetBrains Mono, with `$` prompt prefix in muted color
  - Command: `irm https://raw.githubusercontent.com/WodenJay/HoldRect/main/install.ps1 | iex`
  - Below code block: "Then hold Alt + drag anywhere. That's it." — on-dark-soft
  - Below that: "Or download from GitHub Releases →" text link in coral

### 5. CTA Band (Coral)

- **Background:** Primary coral (`#cc785c`)
- **Padding:** 64px vertical
- **Layout:** Centered text
- **Heading:** "Highlight anything, instantly." — Cormorant Garamond display-sm, on-primary (white)
- **Subtitle:** "Free and open source. MIT licensed." — Inter body-md, white at 90% opacity
- **Button:** Cream/canvas secondary button "Download for Windows" — inverted from primary (canvas bg, ink text)

### 6. Footer

- **Background:** Surface Dark (`#181715`)
- **Padding:** 64px vertical, xl horizontal
- **Layout:** Flex, space-between on desktop; stacked on mobile
- **Left:** "HoldRect" wordmark + "Made by WodenJay" in on-dark-soft
- **Right:** GitHub link + "MIT License" in on-dark-soft
- **Top border:** 1px hairline (`#e6dfd8` at low opacity or a darker variant)

---

## Responsive Breakpoints

| Breakpoint | Width | Changes |
|------------|-------|---------|
| Mobile | < 768px | Hero stacks, features 1-up, nav simplified, text sizes reduce (h1 → 36px, h2 → 28px, h3 → 22px) |
| Tablet | 768–1024px | Features 3-up (cards have min-width 280px, flex-wrap), hero stays 6/6 |
| Desktop | > 1024px | Full layout as designed |
| Wide | > 1440px | Same as desktop, more breathing room, content capped at 1200px |

---

## Google Fonts

Load via CDN `<link>`:
- **Cormorant Garamond**: weights 500 (display headlines)
- **Inter**: weights 400, 500 (body, buttons, labels)
- **JetBrains Mono**: weight 400 (code block)

---

## Deployment

1. Place `index.html`, `style.css`, and `assets/` under `docs/` folder
2. Enable GitHub Pages: Settings → Pages → Source: Deploy from branch `main`, folder `/docs`
3. Page live at `https://wodenjay.github.io/HoldRect/`
4. Keep `index.html` out of repo root to avoid conflicting with `README.md`, `Cargo.toml`, etc.

---

## Accessibility

- Semantic HTML5 (`<nav>`, `<main>`, `<section>`, `<footer>`)
- `alt` text on logo and GIF
- Sufficient color contrast (dark on cream passes WCAG AA)
- `aria-label` on icon-only elements
- Focus-visible styles on buttons and links
- GIF: On `prefers-reduced-motion: reduce`, replace GIF with a static screenshot image. If no static fallback exists, hide the GIF element and show alt-text description instead.

---

## Out of Scope

- JavaScript interactions (no scroll animations, no dark mode toggle)
- Multi-page routing
- Blog or documentation pages
- Analytics or tracking
- Build tooling (no bundler, no preprocessor)
