// A file named main.tsx outside src/ is NOT the composition root.
import { gateway } from '../infrastructure/gateway';

export const NotRoot = () => gateway();
