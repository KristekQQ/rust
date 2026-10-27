import {Group, InstancedMesh, Matrix4, Vector3} from 'three';
import {MAX_STRESS_MODELS, stressPosition} from '../rim/stress.js';
export {MAX_STRESS_MODELS};

// Static Idle-pose load only. Geometry, materials and original gameplay stay owned
// by the source application; this factory owns its instance buffers and roots.
export function createThreeStressLoad(scene, originalRoot) {
    // Freeze the source tree once: its live Idle mixer may keep advancing while
    // the user changes counts or switches between the two representations.
    const template = originalRoot.clone(true);
    template.updateMatrixWorld(true);
    const inverseRoot = template.matrixWorld.clone().invert();
    const parts = [];
    template.traverseVisible(mesh => {
        if (!mesh.isMesh) return;
        if (mesh.isSkinnedMesh || mesh.isInstancedMesh || Object.values(mesh.geometry.morphAttributes).some(a => a.length)) {
            throw new Error('Zátěž podporuje pouze pevné části bez skin/morph animace.');
        }
        const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
        if (materials.some(m => m.transparent)) throw new Error('Průhledné části vyžadují samostatné řazení.');
        parts.push({mesh, local: new Matrix4().multiplyMatrices(inverseRoot, mesh.matrixWorld)});
    });
    if (!parts.length) throw new Error('Model nemá vykreslitelné části.');
    const rotation = template.quaternion.clone(), scale = template.scale.clone();
    let group = new Group(), count = 0, representation = 'instanced', disposed = false;
    group.name = 'Statická zátěž rendereru';
    scene.add(group);
    const position = new Vector3(), rootMatrix = new Matrix4(), matrix = new Matrix4();
    const x = new Vector3(), y = new Vector3(), z = new Vector3();
    function assertSupportedMatrix(value) {
        const a = value.elements;
        x.set(a[0], a[1], a[2]); y.set(a[4], a[5], a[6]); z.set(a[8], a[9], a[10]);
        const size = [x.length(), y.length(), z.length()];
        if (!a.every(Number.isFinite) || size.some(n => n < 1e-8) || value.determinant() <= 0 ||
            Math.abs(x.dot(y)) > 1e-5 * size[0] * size[1] ||
            Math.abs(x.dot(z)) > 1e-5 * size[0] * size[2] ||
            Math.abs(y.dot(z)) > 1e-5 * size[1] * size[2]) {
            throw new Error('Instancování vyžaduje kladné měřítko bez zkosení.');
        }
    }
    function release(target) {
        target.removeFromParent();
        target.traverse(mesh => { if (mesh.isInstancedMesh) mesh.dispose(); });
        target.clear(); // Shared geometry/materials must remain alive.
    }
    function setCount(nextCount, nextRepresentation = 'instanced') {
        if (disposed) throw new Error('Zátěž již byla ukončena.');
        if (!Number.isInteger(nextCount) || nextCount < 0 || nextCount > MAX_STRESS_MODELS) throw new RangeError('Počet modelů musí být 0–2048.');
        if (!['instanced', 'objects'].includes(nextRepresentation)) throw new Error('Neznámý způsob vykreslování.');
        if (nextCount === count && nextRepresentation === representation) return;
        const next = new Group(); next.name = 'Statická zátěž rendereru';
        try {
            if (nextRepresentation === 'objects') {
                for (let i = 0; i < nextCount; i++) {
                    const model = template.clone(true);
                    model.position.set(...stressPosition(i)); model.quaternion.copy(rotation); model.scale.copy(scale);
                    next.add(model);
                    model.traverse(mesh => {if (mesh.isMesh) {mesh.castShadow = false; mesh.receiveShadow = true;}});
                }
            } else if (nextCount) {
                for (const {mesh, local} of parts) {
                    const batch = new InstancedMesh(mesh.geometry, mesh.material, nextCount);
                    batch.name = mesh.name; batch.castShadow = false; batch.receiveShadow = true;
                    batch.renderOrder = mesh.renderOrder; batch.layers.mask = mesh.layers.mask;
                    next.add(batch);
                    for (let i = 0; i < nextCount; i++) {
                        position.set(...stressPosition(i)); rootMatrix.compose(position, rotation, scale);
                        matrix.multiplyMatrices(rootMatrix, local); assertSupportedMatrix(matrix);
                        batch.setMatrixAt(i, matrix);
                    }
                    batch.instanceMatrix.needsUpdate = true;
                    batch.computeBoundingBox(); batch.computeBoundingSphere();
                }
            }
            next.updateMatrixWorld(true);
            next.traverse(node => {node.matrixAutoUpdate = false; node.matrixWorldAutoUpdate = false;});
            scene.add(next);
        } catch (error) {release(next); throw error;}
        release(group); group = next; count = nextCount; representation = nextRepresentation;
    }
    return {
        setCount,
        get count() {return count;},
        get representation() {return representation;},
        get group() {return group;},
        describe() {return {count, representation, partsPerModel: parts.length, staticPose: 'Idle@0', castsShadow: false};},
        dispose() {if (disposed) return; disposed = true; release(group); count = 0;},
    };
}
