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
| `docs/index.html` | Add new `<section class="hero-brand">` inside `<main>`, before the existing hero section |
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
- **Padding:** `var(--space-xxl)` (48px) vertical (compact, not section-level — this is a visual accent, not a full section)
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
      letter-spacing="-2px"
    >HoldRect
      <animate
        attributeName="stroke-dashoffset"
        from="2000" to="0"
        dur="0.8s"
        begin="0s"
        fill="freeze"
        calcMode="spline"
        keySplines="0.25 0.1 0.25 1"
      />
    </text>
  </svg>
</section>
```

**SVG viewBox:** `0 0 800 120` — wide enough for the text at 96px. The `text-anchor="middle"` + `x="50%"` centers it. The SVG scales responsively via `width: 100%; max-width: 800px`.

**Insertion point:** Inside `<main>`, as the first child before the existing `<section class="hero">`.

---

## Animation

### Page Load — Stroke Draw

On page load, the text strokes draw themselves from left to right, then the fill fades in.

**Timeline:**
```
0.0s    stroke-dashoffset: 2000 → 0 begins (0.8s ease-out, SMIL)
0.8s    stroke fully drawn
1.0s    fill-opacity: 0 → 1 begins (0.4s ease, CSS animation)
1.4s    animation complete, text in resting state (filled with stroke)
```

**CSS Keyframes:**

```css
@keyframes hero-brand-fill {
  from { fill-opacity: 0; }
  to   { fill-opacity: 1; }
}
```

**Stroke draw technique:** SVG `<text>` does not support `pathLength`, so `stroke-dasharray: 1` normalization won't work. Instead, use a large `stroke-dasharray` value (2000) that exceeds any glyph's outline length, and animate `stroke-dashoffset` from 2000 to 0 via SMIL `<animate>` (more reliable than CSS on SVG text). The SMIL `<animate>` element is a child of the `<text>` element (included in the HTML snippet above).

Fill opacity is animated via CSS `animation` on the text element, delayed by 1s.

### Hover — Revert to Stroke State

When the user hovers over the `.hero-brand` section:
- `fill-opacity` transitions from `1` to `0` over 0.3s ease
- Stroke remains visible at all times
- The text appears as a stroked/outlined letterform

When the mouse leaves:
- `fill-opacity` transitions from `0` to `1` over 0.3s ease
- Text returns to solid filled state

**Hover target:** The entire `.hero-brand` section (not just the `<text>` glyphs) — this ensures the hover triggers even when the cursor is between letters or in the surrounding whitespace. Use `.hero-brand:hover .hero-brand__text` selector.

**CSS:**

```css
.hero-brand__text {
  fill: #141413;
  fill-opacity: 1;
  stroke: #141413;
  stroke-width: 2;
  stroke-dasharray: 2000;
  stroke-dashoffset: 0;
  transition: fill-opacity 0.3s ease;
}

.hero-brand:hover .hero-brand__text {
  fill-opacity: 0;
}
```

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
    stroke-dashoffset: 0 !important;
    fill-opacity: 1;
    animation: none;
  }
}
```

**Note:** SMIL animations cannot be paused via CSS. The CSS `stroke-dashoffset: 0 !important` overrides the SMIL animation's value. For browsers where SMIL wins over CSS, the text appears fully stroked from the start — acceptable degraded behavior.

### Semantic

- `<section class="hero-brand">` with `aria-label="Brand"`
- SVG has `aria-hidden="true"` (decorative, brand name is already in nav)
- No focusable elements in this band

---

## Responsive

### Desktop (> 768px)
Full 96px text, centered, 48px vertical padding.

### Mobile (< 768px)
- SVG scales naturally via viewBox — set `max-width: 90vw` so text doesn't overflow viewport (800px viewBox at 90vw on 375px screen = 337px rendered width, text ~40px effective — legible)
- Padding reduces to 32px vertical
- Letter-spacing tightens proportionally via viewBox scaling

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
- **Do** use SMIL `<animate>` for the stroke-dashoffset animation with `stroke-dasharray: 2000` (covers all glyph outline lengths since `<text>` lacks `pathLength`)
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
