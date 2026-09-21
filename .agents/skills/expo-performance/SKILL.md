---
name: expo-performance
description: Comprehensive guidelines and best practices for eliminating lag, reducing JS/UI thread bottlenecks, optimizing lists/storage, and monitoring real-device performance with Expo Observe in Expo & React Native apps.
---

# Expo Performance & Lag Reduction Playbook

This skill encodes the definitive practices for achieving buttery-smooth 60/120 FPS performance in Expo & React Native applications based on official Expo engineering guidance and production architecture standards.

---

## 1. Golden Rules of React Native Performance

1. **Keep the UI Thread Free**: Never perform JSON parsing, complex astrology math, crypto operations, or synchronous storage reads on the main thread.
2. **Eliminate Unnecessary Re-Renders**: Treat re-renders as budget expenditures. Isolate rapid state changes to leaf components.
3. **Never Test Performance in Expo Go**: Expo Go runs in interpreted development mode with debug overhead. Always test and profile in **Development Builds** (`expo run:ios` / `expo run:android` / TestFlight release builds).
4. **Instrument in Production**: Monitor real user metrics with `expo-observe` to catch startup, per-screen navigation, and frame drops before users complain.

---

## 2. Component & Render Optimizations

### A. React Compiler & React 19
- Leverage React 19 and React Compiler to automatically memoize computed JSX values and avoid stale memoization bugs.
- For non-compiler components, keep memoization granular (`React.memo`, `useMemo`, `useCallback`).

### B. State Localization (Leaf State)
- **Anti-Pattern**: Putting active gesture position or scroll offset in screen-level React state.
- **Pattern**: Drive gesture/scroll values using `react-native-reanimated` Shared Values (`useSharedValue`) and animated styles (`useAnimatedStyle`) so updates stay 100% on the native/UI thread without triggering JS reconciliations.

```tsx
// ✅ Correct: Driven on UI thread via Reanimated
const translateY = useSharedValue(0);
const animatedStyle = useAnimatedStyle(() => ({
  transform: [{ translateY: translateY.value }],
}));
```

---

## 3. High-Performance Lists & Virtualization

| Use Case | Recommended Solution | Why |
| :--- | :--- | :--- |
| **Large/Infinite Feeds** | `@shopify/flash-list` or `Legend List` | Recycles native cell views instead of unmounting/remounting DOM nodes |
| **Simple Static Stacks** | `AFReportStack` / `View` mapping | Lower overhead than virtualized list when item count < 10 |
| **Avoid Heavy Inline Components** | Extract `renderItem` to standalone `React.memo` component | Prevents re-allocating render callbacks every cycle |

### List Rules:
1. Provide stable `keyExtractor` using unique entity IDs (never index).
2. Set `estimatedItemSize` accurately on `FlashList` to prevent layout thrashing.
3. Keep cell hierarchies shallow—avoid nesting unnecessary containers, backgrounds, or heavy blur views inside list rows.

---

## 4. Storage & Network Caching

### A. Ultra-Fast Key-Value Storage
- Use **MMKV** (`react-native-mmkv`) instead of `AsyncStorage` for synchronous, memory-mapped storage operations (up to 30x faster than SQLite/AsyncStorage bridge calls).
- Keep auth tokens and lightweight local cache in MMKV; keep heavy documents off the critical startup path.

### B. Query Deduplication
- Use **TanStack Query** (`@tanstack/react-query`) with stale-while-revalidate policies.
- Persist queries with `react-query-persist-client` to render cached content immediately on cold launch while background-refreshing.

---

## 5. Background Worklets & Heavy Math

For CPU-heavy routines (ephemeris calculations, panchanga interval slicing, chart division tables):
1. **Reanimated Worklets**: Run computational transforms inside `runOnJS` or worklets when possible.
2. **WebAssembly / Pre-calculated JSON**: In AstroFusion, static astrological lookup tables are vendored as pre-indexed structures rather than computed on-the-fly during scroll interactions.

---

## 6. Real-Device Monitoring with `expo-observe`

### Integration Architecture:
```tsx
import { ObserveRoot, useObserve } from 'expo-observe';

export default function RootLayout() {
  return (
    <ObserveRoot>
      <AppNavigation />
    </ObserveRoot>
  );
}
```

### Key Metrics to Track:
- **Time to First Render (TTR)**: Milliseconds from app process launch to root component mount.
- **Time to Interactive (TTI)**: Recorded via `useObserve().markInteractive()` once initial queries and skeleton states settle.
- **Per-Screen Navigation Transition Time**: Measures route changes and layout mounting lag.
- **Symbolicated Error Traces**: Captures production JS crashes directly into EAS Observe.

---

## 7. Performance Audit Checklist for Expo Apps

- [ ] App uses **Development Build** (`expo-dev-client`) and release mode for benchmarking.
- [ ] No heavy math or JSON operations running synchronously in root context / render functions.
- [ ] Lists with >20 items use recycled list primitives (`FlashList`).
- [ ] Fast key-value reads use **MMKV**.
- [ ] Gestures & fluid sheet transitions run via **Reanimated + Gesture Handler** on UI thread.
- [ ] `expo-observe` is mounted in root layout to track launch and per-screen interaction metrics.
- [ ] Production build runs with zero bare `console.log` / `console.error` overhead.
