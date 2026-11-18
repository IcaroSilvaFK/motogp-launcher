import { expect, it } from 'vitest';
import { covered } from './rule';

it('only exercises one of the two functions', () => {
  expect(covered()).toBe(1);
});
