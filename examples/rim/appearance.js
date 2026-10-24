/** Authored Aurelia display and lighting profile, shared by desktop and XR.
 * @param {import('../../js/engine.js').Engine} engine */
export function configureAppearance(engine){
engine.set_grid_visible(false);engine.set_light_helpers_visible(false);engine.set_render_effects(true,false);
engine.postprocessing.enable('fxaa');
const linear=(/** @type {number} */c)=>{const v=c/255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4};
engine.setBackgroundColor([linear(184),linear(208),linear(202)]);
engine.setSceneAppearance({sky:[linear(229)*2.5,linear(244)*2.5,linear(255)*2.5],ground:[linear(183)*2.5,linear(152)*2.5,linear(104)*2.5],sunDirection:[-45,75,50],exposure:1.25,fogColor:[linear(184),linear(208),linear(202)],fogNear:90,fogFar:245,shadowExtent:95,shadowTarget:[0,0,-15],shadowNear:1,shadowFar:220,shadowNormalBias:0.12,specularAA:true});
}
