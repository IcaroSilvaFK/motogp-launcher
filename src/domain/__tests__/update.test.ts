import { describe, expect, it } from 'vitest';
import { isNewerVersion } from '../update';

describe('isNewerVersion', () => {
  it('detects a newer minor', () => {
    expect(isNewerVersion('1.1.0', '1.0.0')).toBe(true);
  });

  it('detects a newer major and patch', () => {
    expect(isNewerVersion('2.0.0', '1.9.9')).toBe(true);
    expect(isNewerVersion('1.0.1', '1.0.0')).toBe(true);
  });

  it('compares numerically, never lexicographically', () => {
    expect(isNewerVersion('1.10.0', '1.9.0')).toBe(true);
    expect(isNewerVersion('1.9.0', '1.10.0')).toBe(false);
  });

  it('is false for equal versions', () => {
    expect(isNewerVersion('1.1.0', '1.1.0')).toBe(false);
  });

  it('is false for older versions', () => {
    expect(isNewerVersion('1.0.0', '1.1.0')).toBe(false);
    expect(isNewerVersion('0.9.9', '1.0.0')).toBe(false);
  });

  it('tolerates a leading v', () => {
    expect(isNewerVersion('v1.1.0', '1.0.0')).toBe(true);
    expect(isNewerVersion('1.1.0', 'v1.1.0')).toBe(false);
  });

  it('ranks a pre-release below the same release', () => {
    expect(isNewerVersion('1.1.0-beta.1', '1.1.0')).toBe(false);
    expect(isNewerVersion('1.1.0', '1.1.0-beta.1')).toBe(true);
    expect(isNewerVersion('1.1.0-beta.1', '1.0.0')).toBe(true);
  });

  it('is false for equal pre-releases', () => {
    expect(isNewerVersion('1.1.0-beta.1', '1.1.0-beta.1')).toBe(false);
  });

  it('is false when a version is unparseable', () => {
    expect(isNewerVersion('abc', '1.0.0')).toBe(false);
    expect(isNewerVersion('1.1.0', 'abc')).toBe(false);
    expect(isNewerVersion('1.1', '1.0.0')).toBe(false);
    expect(isNewerVersion('', '')).toBe(false);
  });
});
