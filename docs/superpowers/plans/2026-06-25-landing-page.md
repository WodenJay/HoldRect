# HoldRect Landing Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single-page static landing page for HoldRect, styled with the Anthropic design system, deployable on GitHub Pages.

**Architecture:** Two files (`index.html` + `style.css`) plus existing asset files, all under `docs/`. No build step, no JS framework, no dependencies beyond Google Fonts CDN. CSS custom properties hold the design system tokens.

**Tech Stack:** HTML5, CSS3 (custom properties, flexbox, grid), Google Fonts CDN (Cormorant Garamond, Inter, JetBrains Mono)

## Global Constraints

- All files live under `docs/` — never in repo root
- No JavaScript — zero JS files, zero inline scripts
- No build tools — no bundler, no preprocessor, no npm
- English only — all copy text in English
- Anthropic color palette: canvas `#faf9f5`, primary `#cc785c`, dark `#181715`, ink `#141413`
- Fonts: Cormorant Garamond 500 (display), Inter 400/500 (body), JetBrains Mono 400 (code)
- Max content width: 1200px centered
- Section padding: 96px vertical
- Card border radius: 12px; button border radius: 8px; hero card radius: 16px
- Responsive: mobile-first, breakpoints at 768px and 1024px

---

## File Structure

| File | Responsibility |
|------|---------------|
| `docs/index.html` | Semantic HTML markup — 6 bands: nav, hero, features, install, CTA, footer |
| `docs/style.css` | All styles — design system tokens (custom properties), layout, typography, responsive, accessibility |
| `docs/assets/HoldRect.png` | Logo — copy from repo root `assets/HoldRect.png` |
| `docs/assets/HoldRect_show.gif` | Demo GIF — copy from repo root `assets/HoldRect_show.gif` |
| `docs/assets/holdrect-demo-static.png` | Static screenshot fallback — capture a single frame from the GIF |

---

### Task 1: Setup directory and asset files

**Files:**
- Create: `docs/assets/` directory
- Copy: `assets/HoldRect.png` → `docs/assets/HoldRect.png`
- Copy: `assets/HoldRect_show.gif` → `docs/assets/HoldRect_show.gif`
- Create: `docs/assets/holdrect-demo-static.png` (manual step — see below)

- [ ] **Step 1: Create docs directory structure**

```bash
mkdir -p docs/assets
```

- [ ] **Step 2: Copy existing assets into docs/**

```bash
cp assets/HoldRect.png docs/assets/HoldRect.png
cp assets/HoldRect_show.gif docs/assets/HoldRect_show.gif
```

- [ ] **Step 3: Create static screenshot fallback**

Capture a single frame from `HoldRect_show.gif` and save as `docs/assets/holdrect-demo-static.png`. This is used for `prefers-reduced-motion`. Options:
- Use an online GIF-to-PNG tool (e.g., ezgif.com) to extract frame 0
- Or open the GIF in an image editor and export the first frame as PNG
- Or skip for now — the CSS will hide the GIF on reduced-motion and show alt-text as fallback

- [ ] **Step 4: Verify assets exist**

```bash
ls -la docs/assets/
```

Expected: Three files listed — `HoldRect.png`, `HoldRect_show.gif`, `holdrect-demo-static.png` (or two if step 3 was skipped).

- [ ] **Step 5: Commit**

```bash
git add docs/assets/
git commit -m "docs: add assets for landing page"
```

---

### Task 2: Build HTML structure with all content

**Files:**
- Create: `docs/index.html`

**Produces:** Complete semantic HTML with all 6 bands, all copy text, all links, proper `alt` attributes, Google Fonts `<link>` tag.

- [ ] **Step 1: Write `docs/index.html`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>HoldRect — Highlight anything. Instantly.</title>
  <meta name="description" content="A lightweight screen highlighter for recordings, presentations, and live demos. Hold Alt, drag a rectangle, done. Under 2 MB.">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:wght@500&family=Inter:wght@400;500&family=JetBrains+Mono&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="style.css">
  <link rel="icon" href="assets/HoldRect.png" type="image/png">
</head>
<body>

  <!-- 1. Navigation Bar -->
  <nav class="nav" aria-label="Main navigation">
    <div class="nav__inner container">
      <a href="/" class="nav__brand" aria-label="HoldRect home">
        <img src="assets/HoldRect.png" alt="HoldRect logo" class="nav__logo" width="28" height="28">
        <span class="nav__wordmark">HoldRect</span>
      </a>
      <div class="nav__actions">
        <a href="https://github.com/WodenJay/HoldRect" class="nav__link">GitHub</a>
        <a href="https://github.com/WodenJay/HoldRect/releases/latest" class="btn btn--primary">Download</a>
      </div>
    </div>
  </nav>

  <main>

    <!-- 2. Hero Band -->
    <section class="hero" aria-label="Hero">
      <div class="hero__inner container">
        <div class="hero__content">
          <h1 class="hero__title">Highlight anything. Instantly.</h1>
          <p class="hero__subtitle">Hold Alt, drag a rectangle, done. A lightweight screen highlighter for recordings, presentations, and live demos — under 2 MB. Windows today, macOS &amp; Linux coming soon.</p>
          <div class="hero__actions">
            <a href="https://github.com/WodenJay/HoldRect/releases/latest" class="btn btn--primary">Download for Windows</a>
            <a href="https://github.com/WodenJay/HoldRect" class="btn btn--text">View on GitHub →</a>
          </div>
        </div>
        <div class="hero__demo">
          <div class="hero__demo-card">
            <picture>
              <source srcset="assets/holdrect-demo-static.png" media="(prefers-reduced-motion: reduce)">
              <img src="assets/HoldRect_show.gif" alt="HoldRect demo: hold Alt, drag to draw a rainbow-bordered rectangle on screen" class="hero__gif" loading="eager">
            </picture>
          </div>
        </div>
      </div>
    </section>

    <!-- 3. Features Band -->
    <section class="features" aria-label="Features">
      <div class="features__inner container">
        <h2 class="features__heading">Why HoldRect?</h2>
        <div class="features__grid">
          <div class="feature-card">
            <span class="feature-card__icon" aria-hidden="true">⚡</span>
            <h3 class="feature-card__title">Zero-Mode Interaction</h3>
            <p class="feature-card__desc">No toolbar, no hotkey sequence. Hold Alt and drag — that's the entire interface.</p>
          </div>
          <div class="feature-card">
            <span class="feature-card__icon" aria-hidden="true">🌈</span>
            <h3 class="feature-card__title">Rainbow Border</h3>
            <p class="feature-card__desc">Gradient flows along the rectangle perimeter. Unique to HoldRect. Your audience sees exactly what you mean.</p>
          </div>
          <div class="feature-card">
            <span class="feature-card__icon" aria-hidden="true">📌</span>
            <h3 class="feature-card__title">Pin &amp; Spotlight</h3>
            <p class="feature-card__desc">Press 1 to pin the rectangle on screen. Press 2 to dim everything outside. Toggle anytime.</p>
          </div>
          <div class="feature-card">
            <span class="feature-card__icon" aria-hidden="true">🪶</span>
            <h3 class="feature-card__title">Ultra-Lightweight</h3>
            <p class="feature-card__desc">Under 2 MB. Rust native binary with no runtime, no Electron, no installer bloat. You'll forget it's running.</p>
          </div>
        </div>
      </div>
    </section>

    <!-- 4. Install Band (Dark) -->
    <section class="install" aria-label="Installation">
      <div class="install__inner container">
        <h2 class="install__heading">Get started in seconds</h2>
        <div class="install__card">
          <div class="install__code-block">
            <code class="install__code"><span class="install__prompt">$</span> irm https://raw.githubusercontent.com/WodenJay/HoldRect/main/install.ps1 | iex</code>
          </div>
          <p class="install__hint">Then hold Alt + drag anywhere. That's it.</p>
          <a href="https://github.com/WodenJay/HoldRect/releases/latest" class="install__link">Or download from GitHub Releases →</a>
        </div>
      </div>
    </section>

    <!-- 5. CTA Band (Coral) -->
    <section class="cta" aria-label="Call to action">
      <div class="cta__inner container">
        <h2 class="cta__heading">Highlight anything, instantly.</h2>
        <p class="cta__subtitle">Free and open source. MIT licensed.</p>
        <a href="https://github.com/WodenJay/HoldRect/releases/latest" class="btn btn--secondary-on-coral">Download for Windows</a>
      </div>
    </section>

  </main>

  <!-- 6. Footer -->
  <footer class="footer" role="contentinfo">
    <div class="footer__inner container">
      <div class="footer__left">
        <span class="footer__brand">HoldRect</span>
        <span class="footer__credit">Made by <a href="https://github.com/WodenJay" class="footer__link">WodenJay</a></span>
      </div>
      <div class="footer__right">
        <a href="https://github.com/WodenJay/HoldRect" class="footer__link">GitHub</a>
        <span class="footer__sep" aria-hidden="true">·</span>
        <span class="footer__license">MIT License</span>
      </div>
    </div>
  </footer>

</body>
</html>
```

- [ ] **Step 2: Verify HTML is valid**

Open `docs/index.html` in a browser. Expected: page renders with unstyled text, all links work, all images load. The GIF should be visible in the hero section.

- [ ] **Step 3: Commit**

```bash
git add docs/index.html
git commit -m "docs: add landing page HTML structure"
```

---

### Task 3: Write CSS design system tokens and base styles

**Files:**
- Create: `docs/style.css`

**Produces:** CSS custom properties for all design tokens, base reset, typography scale, button styles, container layout.

- [ ] **Step 1: Write CSS custom properties, reset, and base typography**

Create `docs/style.css` with the following content:

```css
/* ============================================================
   Design System Tokens (Anthropic-derived)
   ============================================================ */

:root {
  /* Colors */
  --color-canvas: #faf9f5;
  --color-surface-card: #efe9de;
  --color-surface-dark: #181715;
  --color-surface-dark-elevated: #252320;
  --color-primary: #cc785c;
  --color-primary-active: #a9583e;
  --color-ink: #141413;
  --color-body: #3d3d3a;
  --color-muted: #6c6a64;
  --color-on-primary: #ffffff;
  --color-on-dark: #faf9f5;
  --color-on-dark-soft: #a09d96;
  --color-hairline: #e6dfd8;

  /* Spacing */
  --space-xs: 8px;
  --space-sm: 12px;
  --space-md: 16px;
  --space-lg: 24px;
  --space-xl: 32px;
  --space-xxl: 48px;
  --space-section: 96px;

  /* Border Radius */
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;

  /* Content width */
  --max-width: 1200px;

  /* Typography */
  --font-display: 'Cormorant Garamond', Georgia, 'Times New Roman', serif;
  --font-body: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  --font-code: 'JetBrains Mono', ui-monospace, 'Cascadia Code', 'Source Code Pro', monospace;
}

/* ============================================================
   Reset & Base
   ============================================================ */

*,
*::before,
*::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html {
  font-size: 16px;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  font-family: var(--font-body);
  font-size: 16px;
  font-weight: 400;
  line-height: 1.55;
  color: var(--color-body);
  background-color: var(--color-canvas);
}

img {
  max-width: 100%;
  height: auto;
  display: block;
}

a {
  color: inherit;
  text-decoration: none;
}

/* ============================================================
   Layout
   ============================================================ */

.container {
  width: 100%;
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 0 var(--space-xl);
}

/* ============================================================
   Typography
   ============================================================ */

h1, h2, h3 {
  font-family: var(--font-display);
  font-weight: 500;
  color: var(--color-ink);
  line-height: 1.05;
}

h1 {
  font-size: 64px;
  letter-spacing: -1.5px;
}

h2 {
  font-size: 48px;
  letter-spacing: -1px;
  line-height: 1.1;
}

h3 {
  font-size: 28px;
  letter-spacing: -0.3px;
  line-height: 1.2;
}

/* ============================================================
   Buttons
   ============================================================ */

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-body);
  font-size: 14px;
  font-weight: 500;
  line-height: 1.2;
  border-radius: var(--radius-md);
  padding: 10px 20px;
  border: none;
  cursor: pointer;
  transition: background-color 0.15s ease;
  text-decoration: none;
  white-space: nowrap;
}

.btn--primary {
  background-color: var(--color-primary);
  color: var(--color-on-primary);
}

.btn--primary:hover {
  background-color: var(--color-primary-active);
}

.btn--text {
  background: none;
  color: var(--color-ink);
  padding: 10px 0;
  font-weight: 500;
}

.btn--text:hover {
  color: var(--color-primary);
}

.btn--secondary-on-coral {
  background-color: var(--color-canvas);
  color: var(--color-ink);
}

.btn--secondary-on-coral:hover {
  background-color: var(--color-surface-card);
}
```

- [ ] **Step 2: Verify tokens render**

Open `docs/index.html` in a browser. Expected: page now has cream background, serif h1/h2/h3, Inter body text, coral buttons. Layout is still single-column full-width.

- [ ] **Step 3: Commit**

```bash
git add docs/style.css
git commit -m "docs: add landing page CSS design system tokens"
```

---

### Task 4: Style navigation bar

**Files:**
- Modify: `docs/style.css` (append nav styles)

**Consumes:** CSS custom properties from Task 3, HTML `.nav` classes from Task 2.

- [ ] **Step 1: Append nav styles to `docs/style.css`**

```css
/* ============================================================
   1. Navigation Bar
   ============================================================ */

.nav {
  position: sticky;
  top: 0;
  z-index: 100;
  background-color: var(--color-canvas);
  border-bottom: 1px solid var(--color-hairline);
  height: 64px;
}

.nav__inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 100%;
}

.nav__brand {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
}

.nav__logo {
  width: 28px;
  height: 28px;
}

.nav__wordmark {
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 500;
  color: var(--color-ink);
}

.nav__actions {
  display: flex;
  align-items: center;
  gap: var(--space-lg);
}

.nav__link {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-ink);
}

.nav__link:hover {
  color: var(--color-primary);
}
```

- [ ] **Step 2: Verify nav renders**

Open in browser. Expected: sticky nav at top, logo + "HoldRect" left, "GitHub" link + coral "Download" button right. 64px tall, cream background, subtle bottom border.

- [ ] **Step 3: Commit**

```bash
git add docs/style.css
git commit -m "docs: style navigation bar"
```

---

### Task 5: Style hero band

**Files:**
- Modify: `docs/style.css` (append hero styles)

**Consumes:** CSS custom properties from Task 3, HTML `.hero` classes from Task 2.

- [ ] **Step 1: Append hero styles to `docs/style.css`**

```css
/* ============================================================
   2. Hero Band
   ============================================================ */

.hero {
  background-color: var(--color-canvas);
  padding: var(--space-section) 0;
}

.hero__inner {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-xxl);
  align-items: center;
}

.hero__title {
  margin-bottom: var(--space-lg);
}

.hero__subtitle {
  font-size: 18px;
  line-height: 1.55;
  color: var(--color-body);
  margin-bottom: var(--space-xl);
  max-width: 520px;
}

.hero__actions {
  display: flex;
  align-items: center;
  gap: var(--space-lg);
}

.hero__demo-card {
  background-color: var(--color-surface-dark);
  border-radius: var(--radius-xl);
  padding: var(--space-lg);
  display: flex;
  align-items: center;
  justify-content: center;
}

.hero__gif {
  border-radius: var(--radius-md);
  border: 1px solid var(--color-surface-dark-elevated);
  width: 100%;
  height: auto;
}
```

- [ ] **Step 2: Verify hero renders**

Open in browser. Expected: two-column layout. Left: serif h1, subtitle paragraph, two buttons. Right: GIF inside a dark rounded card. Columns align vertically centered.

- [ ] **Step 3: Commit**

```bash
git add docs/style.css
git commit -m "docs: style hero band"
```

---

### Task 6: Style features band

**Files:**
- Modify: `docs/style.css` (append features styles)

**Consumes:** CSS custom properties from Task 3, HTML `.features` and `.feature-card` classes from Task 2.

- [ ] **Step 1: Append features styles to `docs/style.css`**

```css
/* ============================================================
   3. Features Band
   ============================================================ */

.features {
  background-color: var(--color-surface-card);
  padding: var(--space-section) 0;
}

.features__heading {
  text-align: center;
  margin-bottom: var(--space-xxl);
}

.features__grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-lg);
}

.feature-card {
  background-color: var(--color-canvas);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  padding: var(--space-xl);
}

.feature-card__icon {
  font-size: 32px;
  display: block;
  margin-bottom: var(--space-md);
}

.feature-card__title {
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 500;
  color: var(--color-ink);
  line-height: 1.4;
  letter-spacing: 0;
  margin-bottom: var(--space-xs);
}

.feature-card__desc {
  font-size: 16px;
  line-height: 1.55;
  color: var(--color-body);
}
```

- [ ] **Step 2: Verify features render**

Open in browser. Expected: `#efe9de` band with centered "Why HoldRect?" heading. 2x2 grid of cream cards with emoji, title, description. Each card has hairline border and 12px radius.

- [ ] **Step 3: Commit**

```bash
git add docs/style.css
git commit -m "docs: style features band"
```

---

### Task 7: Style install band (dark)

**Files:**
- Modify: `docs/style.css` (append install styles)

**Consumes:** CSS custom properties from Task 3, HTML `.install` classes from Task 2.

- [ ] **Step 1: Append install styles to `docs/style.css`**

```css
/* ============================================================
   4. Install Band (Dark)
   ============================================================ */

.install {
  background-color: var(--color-surface-dark);
  padding: var(--space-section) 0;
}

.install__heading {
  color: var(--color-on-dark);
  text-align: center;
  margin-bottom: var(--space-xxl);
}

.install__card {
  background-color: var(--color-surface-dark-elevated);
  border-radius: var(--radius-lg);
  padding: var(--space-lg);
  max-width: 720px;
  margin: 0 auto;
}

.install__code-block {
  background-color: var(--color-surface-dark);
  border-radius: var(--radius-md);
  padding: var(--space-md) var(--space-lg);
  overflow-x: auto;
}

.install__code {
  font-family: var(--font-code);
  font-size: 14px;
  line-height: 1.6;
  color: var(--color-on-dark);
  white-space: nowrap;
}

.install__prompt {
  color: var(--color-on-dark-soft);
  margin-right: var(--space-xs);
  user-select: none;
}

.install__hint {
  color: var(--color-on-dark-soft);
  font-size: 16px;
  margin-top: var(--space-lg);
}

.install__link {
  display: inline-block;
  margin-top: var(--space-md);
  color: var(--color-primary);
  font-size: 14px;
  font-weight: 500;
}

.install__link:hover {
  color: var(--color-on-primary);
}
```

- [ ] **Step 2: Verify install band renders**

Open in browser. Expected: dark `#181715` band. Centered "Get started in seconds" in cream serif. Elevated card with code block showing the install command in monospace. Hint text and releases link below.

- [ ] **Step 3: Commit**

```bash
git add docs/style.css
git commit -m "docs: style install band"
```

---

### Task 8: Style CTA band and footer

**Files:**
- Modify: `docs/style.css` (append CTA + footer styles)

**Consumes:** CSS custom properties from Task 3, HTML `.cta` and `.footer` classes from Task 2.

- [ ] **Step 1: Append CTA and footer styles to `docs/style.css`**

```css
/* ============================================================
   5. CTA Band (Coral)
   ============================================================ */

.cta {
  background-color: var(--color-primary);
  padding: 64px 0;
  text-align: center;
}

.cta__heading {
  color: var(--color-on-primary);
  margin-bottom: var(--space-md);
}

.cta__subtitle {
  color: var(--color-on-primary);
  opacity: 0.9;
  font-size: 16px;
  margin-bottom: var(--space-xl);
}

/* ============================================================
   6. Footer
   ============================================================ */

.footer {
  background-color: var(--color-surface-dark);
  border-top: 1px solid rgba(230, 223, 216, 0.15);
  padding: 64px 0;
}

.footer__inner {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.footer__left {
  display: flex;
  align-items: center;
  gap: var(--space-md);
}

.footer__brand {
  font-weight: 500;
  color: var(--color-on-dark);
  font-size: 16px;
}

.footer__credit {
  color: var(--color-on-dark-soft);
  font-size: 14px;
}

.footer__right {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  color: var(--color-on-dark-soft);
  font-size: 14px;
}

.footer__link {
  color: var(--color-on-dark-soft);
  font-size: 14px;
}

.footer__link:hover {
  color: var(--color-on-dark);
}

.footer__sep {
  color: var(--color-on-dark-soft);
  opacity: 0.5;
}

.footer__license {
  color: var(--color-on-dark-soft);
  font-size: 14px;
}
```

- [ ] **Step 2: Verify CTA and footer render**

Open in browser. Expected: coral band with centered "Highlight anything, instantly." in white serif, subtitle below, cream "Download for Windows" button. Footer: dark band with HoldRect/WodenJay left, GitHub/MIT right.

- [ ] **Step 3: Commit**

```bash
git add docs/style.css
git commit -m "docs: style CTA band and footer"
```

---

### Task 9: Add responsive styles and accessibility

**Files:**
- Modify: `docs/style.css` (append responsive + a11y styles)

**Consumes:** All prior CSS. This is the final styling task.

- [ ] **Step 1: Append responsive styles to `docs/style.css`**

```css
/* ============================================================
   Responsive — Mobile (< 768px)
   ============================================================ */

@media (max-width: 768px) {
  h1 {
    font-size: 36px;
  }

  h2 {
    font-size: 28px;
  }

  h3 {
    font-size: 22px;
  }

  .hero {
    padding: var(--space-xxl) 0;
  }

  .hero__inner {
    grid-template-columns: 1fr;
    gap: var(--space-xl);
  }

  .hero__content {
    text-align: center;
  }

  .hero__subtitle {
    max-width: none;
  }

  .hero__actions {
    justify-content: center;
    flex-wrap: wrap;
  }

  .features {
    padding: var(--space-xxl) 0;
  }

  .features__grid {
    grid-template-columns: 1fr;
  }

  .feature-card {
    min-width: auto;
  }

  .install {
    padding: var(--space-xxl) 0;
  }

  .install__code {
    font-size: 12px;
  }

  .footer__inner {
    flex-direction: column;
    gap: var(--space-lg);
    text-align: center;
  }

  .footer__left,
  .footer__right {
    flex-direction: column;
    gap: var(--space-xs);
  }

  /* Mobile nav: hide GitHub link, keep Download button */
  .nav__link {
    display: none;
  }
}

/* ============================================================
   Accessibility
   ============================================================ */

/* Focus visible for keyboard navigation */
:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
  border-radius: var(--radius-md);
}

/* Reduced motion: hide animated GIF, show static fallback */
@media (prefers-reduced-motion: reduce) {
  .hero__gif {
    content: url('assets/holdrect-demo-static.png');
  }
}

/* Ensure sufficient contrast for muted text on dark */
.install__hint,
.install__link {
  /* Already specified in section styles */
}
```

- [ ] **Step 2: Verify responsive behavior**

Open in browser. Resize window:
- **Desktop (> 768px):** Full layout — 2-col hero, 2x2 features grid, all nav items visible.
- **Mobile (< 768px):** Single-column hero stacked, features 1-up, nav shows only logo + Download, footer stacked.

- [ ] **Step 3: Verify accessibility**

- Tab through the page — all interactive elements (links, buttons) should show a coral outline on focus.
- Check images have `alt` text.
- Check `aria-label` on nav, sections.
- Test with browser's "Reduced Motion" emulation — GIF should show static image.

- [ ] **Step 4: Final commit**

```bash
git add docs/style.css
git commit -m "docs: add responsive and accessibility styles"
```

---

### Task 10: Final review and visual polish

**Files:**
- Modify: `docs/style.css` (minor tweaks)
- Modify: `docs/index.html` (minor tweaks if needed)

**Consumes:** Complete page from Tasks 1–9.

- [ ] **Step 1: Full visual review**

Open `docs/index.html` in browser at desktop width. Check every band against the spec:

| Band | Check |
|------|-------|
| Nav | 64px tall, sticky, cream bg, logo+wordmark left, GitHub+Download right |
| Hero | 6/6 grid, serif h1 64px, subtitle 18px, coral button + text link, GIF in dark card |
| Features | `#efe9de` bg, centered serif h2, 2x2 grid, cream cards with hairline border |
| Install | `#181715` bg, cream serif h2, elevated card, monospace code block, hint + releases link |
| CTA | `#cc785c` bg, white serif h2, subtitle, cream button |
| Footer | `#181715` bg, wordmark left, links right, top border |

- [ ] **Step 2: Fix any visual issues**

Common tweaks:
- Adjust spacing if sections feel too tight/loose
- Ensure code block in install band scrolls horizontally on narrow screens (already has `overflow-x: auto`)
- Check GIF border looks good against dark card background

- [ ] **Step 3: Final commit**

```bash
git add docs/
git commit -m "docs: landing page complete"
```
