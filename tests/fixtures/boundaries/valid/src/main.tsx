// Authorized composition root: may import infrastructure and ui.
import { gateway } from './infrastructure/gateway';
import { Good } from './ui/Good';

export const wired = [gateway, Good];
