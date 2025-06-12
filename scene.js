export class Scene {
    constructor(addLightFunc, clearLightsFunc, addCubeFunc, clearCubesFunc) {
        this.addLight = addLightFunc;
        this.clearLights = clearLightsFunc;
        this.addCube = addCubeFunc;
        this.clearCubes = clearCubesFunc;
    }

    addCubeDefault(opts = {}) {
        const {
            x = 0, y = 0, z = 0,
            r = 1, g = 1, b = 1,
            scale = 1,
        } = opts;
        this.addCube(x, y, z, r, g, b, scale);
    }

    addLightDefault(opts = {}) {
        const { x = 1.5, y = 1.0, z = 2.0, r = 1.0, g = 1.0, b = 1.0 } = opts;
        this.addLight(x, y, z, r, g, b);
    }
}
