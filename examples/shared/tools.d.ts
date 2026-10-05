import type { Engine } from '../../js/engine.js';

export interface ToolsOptions {
    title: string;
    defaults: {
        shadows: boolean;
        mirror: boolean;
        grid: boolean;
        helpers: boolean;
    };
    onReset(): void;
}

export function createTools(
    container: HTMLElement | null,
    engine: Engine,
    options: ToolsOptions,
): { dispose(): void };
