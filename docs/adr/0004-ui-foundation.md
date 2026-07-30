# ADR 0004: UI tokens, components, and directional isolation

- Status: accepted
- Date: 2026-07-29

## Decision

The desktop UI uses role-based design tokens for spacing, typography, shape,
borders, focus, elevation, and semantic states. Light and dark palettes assign
those roles; the system theme follows the operating system preference.

Reusable visual primitives live in `apps/desktop/src/lib/components`. They may
depend on Svelte and other visual primitives, but not on Tauri, application API
adapters, generated contracts, repositories, or scheduler code. Screen
components compose those primitives and own feature-specific behavior.

The application shell has a fixed interface direction. User content carries its
own `lang` and `dir` metadata, uses the content font fallback, and is isolated
from surrounding controls with Unicode bidi isolation. An RTL card therefore
does not reverse navigation, labels, shortcuts, or grading controls.

Primary screens expose one emphasized action and use progressive disclosure for
advanced settings. Empty, loading, and error states use the same feedback
primitives. Motion is reduced when the operating system requests it.

## Verification

Browser tests cover keyboard operation, screen-reader labels, responsive
layouts, theme contrast, visible focus, reduced motion, state feedback, and
native dialog dismissal. Stable screenshot baselines cover LTR, RTL, CJK, and
mixed-script study content.

## Consequences

Feature screens can evolve independently while retaining a coherent visual and
accessibility contract. New semantic colors or layout values must enter through
tokens rather than one-off literals. Content direction remains explicit at the
rendering boundary instead of leaking into application chrome.
