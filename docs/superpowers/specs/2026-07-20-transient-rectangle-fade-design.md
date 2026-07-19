# Transient Rectangle Fade-Out Design

## Goal

After a non-pinned Alt + left-drag ends, keep its border visible and fade it out over 300 ms instead of hiding it immediately. The effect must preserve HoldRect's low-memory background behavior and existing interaction semantics.

## Confirmed Behavior

- Only transient borders fade.
- A fade starts when the user releases the left mouse button or releases the configured modifier while dragging.
- The duration is 300 ms.
- Opacity remains stronger early and disappears faster near the end.
- Previous borders continue fading while the user starts another drag; multiple fades may coexist.
- Rainbow colors continue flowing during the fade.
- Pinned rectangles remain fully visible and do not fade.
- A transient Spotlight mask ends immediately; only its border fades.
- Escape remains cancellation and removes the active border immediately.
- A zero-width or zero-height rectangle does not create a fade.

## Architecture

Fade state belongs to the overlay rendering lifecycle, not the input state machine. `src/state.rs`, `DrawingState`, `AppState`, and `process_event` remain unchanged.

`overlay::App` owns a `Vec<FadingRect>`. Each entry stores only:

- normalized screen coordinates `(x0, y0, x1, y1)`;
- the monotonic `Instant` at which the fade started.

The overlay retains geometry rather than DIB pixels. This reuses the current clear-and-redraw pipeline, keeps each fade to a few machine words, survives DIB recreation, and allows the rainbow animation to continue.

A separate `FadeManager` abstraction is not needed. The collection has one owner and one consumer, so direct ownership by `App` is simpler.

## Event Flow

`App::about_to_wait` evaluates fade creation inside the existing per-event channel-drain loop.

For every input event:

1. Read the pre-transition `DrawingState`, geometry, and `pinned_active` value.
2. Run the existing pure `process_event` transition.
3. If that individual event ended a non-pinned drawing, append a `FadingRect`.

The final geometry is selected as follows:

- `MouseButtonUp { x, y }`: use the event's `(x, y)` as the final corner so the fade matches the actual release position.
- `ModifierChanged { pressed: false }`: use the last `current` position stored by `DrawingState::Drawing`.
- `EscapePressed`: do not create a fade.

Fade detection must happen per event rather than once around the entire drain loop. A batch can contain mouse movement, mouse-up, and modifier-release events, and the transition that ended the drawing must not be lost.

Pinned completion follows the existing path and creates no fade. A later mouse-up after modifier-release sees a non-drawing state, so it cannot create a duplicate fade.

## Fade Curve

Let:

```text
p = clamp(elapsed / 300 ms, 0, 1)
alpha = round(255 × (1 - p²))
```

This makes the border remain visually strong early and then disappear faster near the end. Expected reference values are:

- 0 ms: 255;
- 150 ms: approximately 191;
- 300 ms or later: 0.

`Instant` is used instead of wall-clock time. One `now` value is captured per frame and passed to fade calculations so all entries are evaluated consistently and tests can supply deterministic times.

Expired entries are removed before overlay visibility and event-loop control-flow decisions.

## Rendering and Composition

The existing border pixel pipeline gains an alpha input. Full-opacity pinned and active borders pass `255`; fading borders pass their calculated alpha.

`UpdateLayeredWindow` with `AC_SRC_ALPHA` expects premultiplied color channels for translucent pixels. Fading border pixels therefore use premultiplied RGB and source-over composition rather than replacing destination RGBA values directly. This prevents a fading border from cutting a transparent seam through an existing Spotlight mask.

The DIB render order is:

1. Spotlight mask;
2. fading borders, oldest to newest;
3. pinned borders;
4. the active drawing border.

This ordering keeps pinned and active borders fully opaque at intersections. Newer fades naturally appear above older fades.

Every fading rainbow border uses the current frame's existing `time_offset`; the animation does not freeze at release.

## Overlay Lifecycle

A non-empty fade collection counts as visible overlay content and active animation:

- `should_show_overlay` includes active fades;
- the event loop continues using the existing 16 ms `WaitUntil` cadence while any fade remains;
- the DIB and layered window stay alive while only fading borders are visible.

After the last fade expires, if there is no active drawing and no pinned rectangle, the existing hide path submits a transparent frame, hides the overlay, and releases the DIB cache. No background timer or continuous animation remains active afterward.

## Testing Strategy

Implementation follows red-green-refactor TDD. Tests are added before production changes and must demonstrate failure against the current immediate-hide behavior.

### Event capture

- Mouse-up uses its final event coordinates.
- Modifier release uses the last drawing coordinates.
- Pinned completion creates no fade.
- Escape creates no fade.
- Idle/armed events create no fade.
- Zero-width and zero-height rectangles create no fade.
- Batched events are evaluated individually.
- Modifier release followed by mouse-up creates one fade, not two.

### Timing and collection lifecycle

- Alpha is 255 at 0 ms, approximately 191 at 150 ms, and 0 at or after 300 ms.
- Alpha is monotonically non-increasing.
- Multiple entries retain independent start times.
- Expired entries are removed using a supplied frame time.

### Pixel composition and ordering

- Translucent colored pixels use premultiplied RGB.
- Source-over blending is correct over a transparent destination.
- Source-over blending is correct over the Spotlight black mask.
- Pinned borders remain opaque where they cross a fading border.
- The active drawing remains opaque where it crosses a fading border.
- Multiple fades compose oldest to newest.
- Existing full-opacity border geometry and color tests continue to pass.

### Visibility and scheduling

- An overlay containing only fades remains visible.
- A non-empty fade collection keeps the 16 ms animation cadence active.
- The overlay becomes hidden after the final fade expires when no other overlay content exists.

Final verification uses the smallest relevant Windows test target first with Cargo concurrency limited to one, followed by the complete test suite with one job. Manual verification covers normal mouse-up, modifier-release, rapid consecutive drags, overlap with pinned borders, and overlap with an existing pinned Spotlight.

## Error Handling and Limits

The feature introduces no I/O, dependency, thread, or new fallible platform API. Existing DIB allocation and layered-window failure handling remain unchanged.

The fade vector has no arbitrary item cap. Entries live for only 300 ms and are created by completed human drag gestures, so a configured cap would add behavior and code without a practical memory benefit.

## Non-Goals

- No fade duration or curve configuration.
- No fade for pinned rectangles when Escape clears them.
- No fade for Escape cancellation.
- No Spotlight-mask fade.
- No pixel or screen snapshots.
- No changes to the input state model.
- No new cross-platform renderer; this iteration follows the current Windows-first overlay implementation.
