# Hero Stroke Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an animated "HoldRect" text band with stroke-draw animation above the existing hero section.

**Architecture:** Two files modified (`docs/index.html` and `docs/style.css`). An inline SVG `<text>` element with SMIL `<animate>` for the stroke-draw effect, CSS `@keyframes` for the fill fade-in, and CSS `transition` for the hover fill-opacity toggle. No JavaScript.

**Tech Stack:** HTML5, SVG (SMIL animation), CSS3 (custom properties, keyframes, transitions, media queries)

## Global Constraints

- No JavaScript — zero JS files, zero inline scripts
- No new files — modify `docs/index.html` and `docs/style.css` only
- Insertion point: inside `<main>`, as the first child before `<section class="hero">`
- Monochrome: ink (#141413) for stroke and fill, canvas (#faf9f5) background — no coral, no rainbow
- Stroke-dasharray: 2000 (covers all glyph outlines since `<text>` lacks `pathLength`)
- SMIL `<animate>` for stroke-dashoffset (not CSS, which is unreliable on SVG text)
- Hover target: `.hero-brand:hover .hero-brand__text` (whole section, not just text glyphs)
- Reduced motion: CSS animations suppressed via `prefers-reduced-motion`. SMIL cannot be paused by CSS — stroke-draw will still play in some browsers; this is acceptable degraded behavior
- Font: Cormorant Garamond **500** (per Anthropic design system: this weight approximates Copernicus at 400). Already loaded via Google Fonts CDN but needs `wght@500` added to the `<link>`
- Hover has no effect during the 0–1.4s animation window (CSS animation overrides transition)
- Commit as WodenJay, no Co-Author line

---

## File Structure

| File | Responsibility |
|------|---------------|
| `docs/index.html` | Update Google Fonts `<link>` to include `wght@500` for Cormorant Garamond. Insert `<section class="hero-brand">` with inline SVG inside `<main>`, before `<section class="hero">` |
| `docs/style.css` | Append sections 15–17 after line 533 (end of section 14): layout, SVG text styles, keyframe animation, hover state, reduced-motion override, mobile responsive |

---

### Task 1: Add hero-brand HTML and CSS

**Files:**
- Modify: `docs/index.html` (Google Fonts link + insert SVG section)
- Modify: `docs/style.css` (append after line 533, the closing `}` of section 14)

**Produces:** Fully functional animated "HoldRect" text band with stroke-draw on load, fill fade-in, hover toggle, reduced-motion fallback, and mobile responsive.

- [ ] **Step 1: Update Google Fonts link in `docs/index.html`**

The current link (line 10) loads `Cormorant+Garamond:wght@400`. Update to include weight 500:

```html
<link href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:wght@400;500&family=Inter:wght@400;500&family=JetBrains+Mono&display=swap" rel="stylesheet">
```

- [ ] **Step 2: Insert SVG section into `docs/index.html`**

Insert the following after the empty line 31 (after `<main>`) and before line 32 (`<!-- 2. Hero Band -->`):

```html

    <!-- 1.5. Hero Brand -->
    <section class="hero-brand" aria-label="Brand">
      <svg class="hero-brand__svg" viewBox="0 0 800 120" aria-hidden="true">
        <text
          class="hero-brand__text"
          x="50%"
          y="50%"
          dominant-baseline="central"
          text-anchor="middle"
          font-family="'Cormorant Garamond', Georgia, 'Times New Roman', serif"
          font-size="96"
          font-weight="500"
          letter-spacing="-0.02em"
        >HoldRect
          <animate
            attributeName="stroke-dashoffset"
            from="2000" to="0"
            dur="0.8s"
            begin="0s"
            fill="freeze"
            calcMode="spline"
            keyTimes="0;1"
            keySplines="0.25 0.1 0.25 1"
          />
        </text>
      </svg>
    </section>
```

- [ ] **Step 3: Append hero-brand CSS to `docs/style.css`**

Append the following after line 533 (the closing `}` of the section 14 `@media` block):

```css
/* --- 15. Hero Brand --- */
.hero-brand {
  background-color: var(--color-canvas);
  padding: 64px 0;
  display: flex;
  justify-content: center;
}

.hero-brand__svg {
  width: 100%;
  max-width: 800px;
  height: auto;
  display: block;
}

.hero-brand__text {
  fill: var(--color-ink);
  fill-opacity: 0;
  stroke: var(--color-ink);
  stroke-width: 2;
  font-weight: 500;
  stroke-dasharray: 2000;
  stroke-dashoffset: 2000;
  animation: hero-brand-fill 0.4s ease 1s forwards;
  transition: fill-opacity 0.3s ease;
}

@keyframes hero-brand-fill {
  from { fill-opacity: 0; }
  to   { fill-opacity: 1; }
}

.hero-brand:hover .hero-brand__text {
  fill-opacity: 0;
}

/* --- 16. Hero Brand Responsive --- */
@media (max-width: 768px) {
  .hero-brand {
    padding: var(--space-xl) 0;
  }
  .hero-brand__svg {
    max-width: 90vw;
  }
}

/* --- 17. Hero Brand Reduced Motion --- */
@media (prefers-reduced-motion: reduce) {
  .hero-brand__text {
    stroke-dashoffset: 0 !important;
    fill-opacity: 1;
    animation: none;
  }
}
```

**Key implementation details:**
- `fill-opacity: 0` initially — text invisible until stroke draws
- `stroke-dasharray: 2000` + `stroke-dashoffset: 2000` — stroke fully hidden
- SMIL `<animate>` draws stroke from 2000→0 over 0.8s (starts immediately on load)
- CSS `animation: hero-brand-fill` delays 1s then fades fill in over 0.4s
- `transition: fill-opacity 0.3s ease` on the base rule — both hover-in and hover-out get 0.3s
- `.hero-brand:hover .hero-brand__text` sets `fill-opacity: 0` (back to stroke-only outline)
- The `animation` `forwards` fill mode sets `fill-opacity: 1` after completion; `transition` then governs hover changes
- **Hover during animation (0–1.4s):** no effect — CSS animation overrides transition. This is expected
- **Reduced motion:** CSS `animation: none` suppresses the fill fade. SMIL stroke-draw cannot be paused by CSS — it will still play in some browsers. `stroke-dashoffset: 0 !important` overrides the SMIL value where CSS wins. Acceptable degraded behavior

- [ ] **Step 4: Verify in browser**

Open `docs/index.html`. Expected behavior:
1. Page loads → stroke draws from left to right over 0.8s
2. At 1.0s → fill fades in over 0.4s
3. Hover over the "HoldRect" band → fill disappears, text becomes outlined
4. Mouse leaves → fill reappears over 0.3s
5. On mobile (< 768px) → text scales down proportionally, padding reduces
6. With `prefers-reduced-motion: reduce` → text appears fully filled immediately (fill animation suppressed; SMIL stroke may still play)

- [ ] **Step 5: Commit**

```bash
git add docs/index.html docs/style.css
git commit -m "feat: add hero-brand stroke animation band"
```
