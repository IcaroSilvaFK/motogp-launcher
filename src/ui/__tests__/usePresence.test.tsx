import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ENTER_DURATION_MS, EXIT_DURATION_MS, prefersReducedMotion, stagger, STAGGER_CAP } from '../motion';
import { usePresence } from '../usePresence';

function stubReducedMotion(reduce: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: reduce })),
  );
}

describe('usePresence', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    stubReducedMotion(false);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  const mount = (initial: string | null) =>
    renderHook(({ value }: { value: string | null }) => usePresence(value), { initialProps: { value: initial } });

  it('renders nothing while the value is null', () => {
    const { result } = mount(null);
    expect(result.current.item).toBeNull();
  });

  it('marks a new value as entering, then entered', () => {
    const { result } = mount('a');
    expect(result.current).toEqual({ item: 'a', state: 'entering' });
    act(() => vi.advanceTimersByTime(ENTER_DURATION_MS));
    expect(result.current).toEqual({ item: 'a', state: 'entered' });
  });

  it('keeps the last value while exiting and drops it after the exit duration', () => {
    const { result, rerender } = mount('a');
    rerender({ value: null });
    expect(result.current).toEqual({ item: 'a', state: 'exiting' });
    act(() => vi.advanceTimersByTime(EXIT_DURATION_MS - 1));
    expect(result.current.item).toBe('a');
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.item).toBeNull();
  });

  it('enters again when a value comes back during the exit', () => {
    const { result, rerender } = mount('a');
    rerender({ value: null });
    rerender({ value: 'b' });
    expect(result.current).toEqual({ item: 'b', state: 'entering' });
    act(() => vi.advanceTimersByTime(EXIT_DURATION_MS));
    expect(result.current.item).toBe('b');
  });

  it('swaps the content in place when the value changes', () => {
    const { result, rerender } = mount('a');
    act(() => vi.advanceTimersByTime(ENTER_DURATION_MS));
    rerender({ value: 'b' });
    expect(result.current).toEqual({ item: 'b', state: 'entered' });
  });

  it('unmounts at once under reduced motion', () => {
    stubReducedMotion(true);
    const { result, rerender } = mount('a');
    rerender({ value: null });
    expect(result.current.item).toBeNull();
  });

  it('unmounts at once when the exit duration is zero', () => {
    const { result, rerender } = renderHook(({ value }: { value: string | null }) => usePresence(value, 0), {
      initialProps: { value: 'a' as string | null },
    });
    rerender({ value: null });
    expect(result.current.item).toBeNull();
  });
});

describe('motion helpers', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('caps the stagger index', () => {
    expect(stagger(0)).toEqual({ '--stagger-index': 0 });
    expect(stagger(3)).toEqual({ '--stagger-index': 3 });
    expect(stagger(STAGGER_CAP + 5)).toEqual({ '--stagger-index': STAGGER_CAP });
    expect(stagger(-1)).toEqual({ '--stagger-index': 0 });
  });

  it('treats a missing matchMedia as reduced motion', () => {
    vi.stubGlobal('matchMedia', undefined);
    expect(prefersReducedMotion()).toBe(true);
  });

  it('reads the reduced-motion media query', () => {
    stubReducedMotion(false);
    expect(prefersReducedMotion()).toBe(false);
    stubReducedMotion(true);
    expect(prefersReducedMotion()).toBe(true);
  });
});
