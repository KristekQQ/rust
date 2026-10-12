import type { Engine } from '../../js/engine.js';
export function createUI(container: HTMLElement | null, engine: Engine, options?: { resetView?: () => void }): { dispose(): void };
