# HoldRect Hero Stroke Animation — Design Spec

**Date:** 2026-06-25
**Scope:** Add animated "HoldRect" text band above the existing hero section on the landing page
**Style:** Anthropic design system (Cormorant Garamond serif, ink on canvas)

---

## Overview

A new band inserted between the navigation bar and the existing hero section. Displays "HoldRect" in large serif type with a stroke-draw animation on page load and a stroke/fill toggle on hover. The animation evokes the product's core interaction — drawing a rectangle — applied to the brand name itself.

---

## Files Modified

| File | Change |
|------|--------|
| `docs/index.html` | Add new `<section class="hero-brand">` between nav and hero |
| `docs/style.css` | Add `.hero-brand` layout, SVG text styles, keyframe animations, hover states, reduced-motion fallback |

No new files. No JavaScript. Pure CSS animation on an SVG `<text>` element.

---

## Layout

```
nav (64px, sticky)
┌─────────────────────────────┐
│                             │  padding: 48px vertical
│       HoldRect (96px)       │  canvas bg (#faf9f5)
│       (stroke → fill)       │  centered
│                             │
└─────────────────────────────┘
┌─────────────────────────────┐
│  Highlight anything...      │  existing hero band
│  [GIF demo]                 │
└─────────────────────────────┘
```

- **Background:** Canvas (`#faf9f5`) — same as hero
- **Padding:** 48px vertical (compact, not section-level — this is a visual accent, not a full section)
- **Alignment:** Centered horizontally
- **Max-width:** Follows `.container` (1200px)

---

## Typography

| Property | Value |
|----------|-------|
| Font family | Cormorant Garamond, Georgia, "Times New Roman", serif |
| Font size | 96px |
| Font weight | 400 |
| Letter spacing | -2px |
| Stroke width | 2px |
| Stroke color | Ink (`#141413`) |
| Fill color | Ink (`#141413`) |

---

## SVG Implementation

The text is rendered as an inline SVG `<text>` element, not as HTML text. This gives native control over `stroke-dasharray` and `stroke-dashoffset` for the draw animation.

```html
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
      font-weight="400"
      letter-spacing="-2"
    >HoldRect</text>
  </svg>
</section>
```

**SVG viewBox:** `0 0 800 120` — wide enough for the text at 96px. The `text-anchor="middle"` + `x="50%"` centers it. The SVG scales responsively via `width: 100%; max-width: 800px`.

---

## Animation

### Page Load — Stroke Draw

On page load, the text strokes draw themselves from left to right, then the fill fades in.

**Timeline:**
```
0.0s    stroke-dashoffset: 100% → 0% begins (0.8s ease-out)
0.8s    stroke fully drawn
1.0s    fill-opacity: 0 → 1 begins (0.4s ease)
1.4s    animation complete, text in resting state (filled with stroke)
```

**CSS Keyframes:**

```css
@keyframes hero-brand-draw {
  from { stroke-dashoffset: 1; }
  to   { stroke-dashoffset: 0; }
}

@keyframes hero-brand-fill {
  from { fill-opacity: 0; }
  to   { fill-opacity: 1; }
}
```

**Implementation detail:** `stroke-dasharray` is set to `1` and `stroke-dashoffset` animates from `1` to `0` (using `pathLength` normalization via CSS `stroke-dasharray: 1` and `stroke-dashoffset: 1` on the text element). CSS cannot animate `stroke-dashoffset` on SVG `<text>` in all browsers — use `@supports` or fall back to a SMIL `<animate>` element inside the SVG for `stroke-dashoffset`. The SMIL approach is more reliable for SVG text stroke animation:

```xml
<animate
  attributeName="stroke-dashoffset"
  from="1" to="0"
  dur="0.8s"
  begin="0s"
  fill="freeze"
  calcMode="spline"
  keySplines="0.25 0.1 0.25 1"
/>
```

Fill opacity is animated via CSS `animation` on the same text element, delayed by 1s.

### Hover — Revert to Stroke State

When the user hovers over the text:
- `fill-opacity` transitions from `1` to `0` over 0.3s ease
- Stroke remains visible at all times
- The text appears as a stroked/outlined letterform

When the mouse leaves:
- `fill-opacity` transitions from `0` to `1` over 0.4s ease
- Text returns to solid filled state

**CSS:**

```css
.hero-brand__text {
  fill: #141413;
  fill-opacity: 1;
  stroke: #141413;
  stroke-width: 2;
  stroke-dasharray: 1;
  stroke-dashoffset: 0;
  transition: fill-opacity 0.3s ease;
}

.hero-brand__text:hover {
  fill-opacity: 0;
}
```

The `transition` on `fill-opacity` handles both hover-in and hover-out (with different durations if needed via `transition` shorthand).

---

## Accessibility

### Reduced Motion

On `prefers-reduced-motion: reduce`:
- Skip the stroke-draw animation entirely
- Show text in filled state immediately (fill-opacity: 1, stroke-dashoffset: 0)
- Hover effect still works (it's a simple opacity transition, not a large motion)

```css
@media (prefers-reduced-motion: reduce) {
  .hero-brand__text {
    stroke-dashoffset: 0;
    fill-opacity: 1;
    animation: none;
  }
  .hero-brand__text animate {
    /* SMIL animation disabled via CSS cannot directly control SMIL,
       but setting stroke-dashoffset: 0 via CSS overrides the animated value */
  }
}
```

**Note:** SMIL animations cannot be paused via CSS. The CSS `stroke-dashoffset: 0` with `!important` will override the SMIL animation in browsers that support CSS overrides on presentation attributes. For browsers where SMIL wins, the text simply appears fully stroked from the start — acceptable degraded behavior.

### Semantic

- `<section class="hero-brand">` with `aria-label="Brand"`
- SVG has `aria-hidden="true"` (decorative, brand name is already in nav)
- No focusable elements in this band

---

## Responsive

### Desktop (> 768px)
Full 96px text, centered, 48px vertical padding.

### Mobile (< 768px)
- Font size scales down to 56px — add a second `<text>` at 56px inside a `<switch>` with `requiredFeature`, or simpler: set SVG `width: 100%; max-width: 80vw` so the viewBox scales the text proportionally on small screens (the 800-wide viewBox at 80vw on 375px = 300px rendered width, text ~36px effective). Adjust `max-width` in the media query to control the minimum readable size.
- Padding reduces to 32px vertical
- Letter-spacing tightens slightly

```css
@media (max-width: 768px) {
  .hero-brand {
    padding: var(--space-xl) 0;
  }
  .hero-brand__svg {
    max-width: 90vw;
  }
}
```

---

## Color

| Element | Color | Token |
|---------|-------|-------|
| Background | `#faf9f5` | Canvas |
| Stroke | `#141413` | Ink |
| Fill | `#141413` | Ink |

Monochrome. No coral. No rainbow. The animation IS the visual interest — color discipline keeps the focus on the stroke draw.

---

## Do's and Don'ts

- **Do** keep the stroke width at 2px — thinner gets invisible at smaller sizes, thicker looks clunky
- **Do** use SMIL for the stroke-dashoffset animation (more reliable than CSS on SVG text)
- **Do** respect prefers-reduced-motion
- **Don't** add JavaScript — this is pure SVG + CSS
- **Don't** use coral or rainbow colors on this element — the rest of the page has color, this band is typographic
- **Don't** make the hover effect too aggressive — the fill-opacity transition is subtle and elegant
- **Don't** animate letter-spacing or transform on hover — that's motion, not the stroke concept

---

## Out of Scope

- JavaScript animation libraries (GSAP, anime.js)
- Scroll-triggered animation (plays on load only)
- Color changes on hover (stays monochrome)
- Changes to existing hero section
