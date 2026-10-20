/** @param {import('../../js/engine.d.ts').Scene} scene */
export function createScene(scene) {
    return ['01234','56789'].map((value,index)=>scene.createNumericText(value,{
        position:[0,2.1-index*1.25,0],width:6.2,height:.85,
    }).setScale([-1,1,1]).setEmission(5));
}
