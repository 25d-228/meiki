# ADR 0004: shadcn-svelte UI and directional isolation

- Status: accepted
- Date: 2026-07-30

## Decision

The desktop UI uses shadcn-svelte generated components, Tailwind CSS, and
shadcn CSS variables. The neutral base palette supports light and dark classes;
the system theme follows the operating system preference.

The generated component source lives in
`apps/desktop/src/lib/components/ui`. Screens import those components directly
instead of maintaining wrappers with parallel variants or theme tokens. One
`components.json` owns the standard `$lib` aliases, and `cn` combines component
classes. Native elements remain appropriate for simple controls.

The application shell has a fixed interface direction. User content carries its
own `lang` and `dir` metadata, uses the content font fallback, and is isolated
from surrounding controls with Unicode bidi isolation. An RTL card therefore
does not reverse navigation, labels, shortcuts, or grading controls.

Primary screens expose one emphasized action and use progressive disclosure for
advanced settings. Empty, loading, and error states use Cards and Alerts.
Destructive confirmations use AlertDialog rather than browser dialogs. Motion
is reduced when the operating system requests it.

## Verification

Browser tests cover keyboard operation, screen-reader labels, responsive
layouts, theme contrast, visible focus, reduced motion, state feedback, dialog
focus trapping/restoration, and 200% zoom-equivalent reflow. Stable screenshot
baselines cover every primary screen, prompt and reveal states, light and dark
themes, 640×720, 960×720, and 1440×900 viewports, LTR, RTL, CJK, combining-mark,
and mixed-script content, plus empty, loading, error, and destructive states.

The boundary check rejects the removed custom component files, token and theme
stylesheets, browser confirmations, non-standard component aliases, and a
missing required generated component set.

## Consequences

The repository owns generated shadcn-svelte source but does not own a second
component framework. Component updates are explicit source changes. Small
screen-local styles remain only for feature layout and learning-content
semantics. Content direction remains explicit at the rendering boundary instead
of leaking into application chrome.
