---
name: accessibility-review
description: Run a WCAG 2.1 AA accessibility audit on a design or page. Trigger with "audit accessibility", "check a11y", "is this accessible?", or when reviewing a design for color contrast, keyboard navigation, touch target size, or screen reader behavior before handoff.
---

# Accessibility Review (WCAG 2.1 AA)

Audit a design, mobile screen, or page for WCAG 2.1 AA accessibility compliance.

## WCAG 2.1 AA Quick Reference

### Perceivable
- **1.1.1** Non-text content has alt text / accessibilityLabel
- **1.3.1** Info and structure conveyed semantically (roles, headers, groups)
- **1.4.3** Contrast ratio >= 4.5:1 (normal text), >= 3:1 (large text)
- **1.4.11** Non-text contrast >= 3:1 (UI components, focus rings, graphics)

### Operable
- **2.1.1** All functionality available via keyboard / switch control / screen reader
- **2.4.3** Logical focus order
- **2.4.7** Visible focus indicator
- **2.5.5** Touch target >= 44x44 CSS pixels (or 44x44 dp in React Native)

### Understandable
- **3.2.1** Predictable on focus (no unexpected state changes)
- **3.3.1** Error identification (describe the error clearly)
- **3.3.2** Labels or instructions for inputs

### Robust
- **4.1.2** Name, role, value for all UI components (`accessibilityRole`, `accessibilityState`, `accessibilityLabel`, `accessibilityHint`)

## Testing Approach

1. Automated scan / static audit
2. Keyboard & Switch navigation / Tab order
3. Screen reader testing (VoiceOver / TalkBack attributes: `accessibilityLabel`, `accessibilityHint`, `accessibilityRole`, `accessibilityState`)
4. Color contrast verification against dark/glass theme backgrounds
5. Font scaling (`allowFontScaling`, `maxFontSizeMultiplier`) and Reduced Motion (`useReducedMotion()`)
