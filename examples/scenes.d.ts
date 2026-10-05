import type {Scene} from '../js/engine.js';
export function basic(scene:Scene):void;
export function shadows(scene:Scene):void;
export function mirror(scene:Scene):void;
export function animatedLights(scene:Scene):void;
export function culling(scene:Scene):void;
export const examples:readonly {id:string;title:string;description:string;build:(scene:Scene)=>void;shadows:boolean;mirror:boolean;grid:boolean;helpers:boolean;camera:'orbit'|'free'}[];
