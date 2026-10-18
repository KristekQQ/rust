// Offline 90-degree camera bake, not an equirectangular projection of a photo.
import {PNG} from 'pngjs';
import {readFile, writeFile} from 'node:fs/promises';
import {faces,projectArenaRay} from './environment-projection.mjs';
const root = new URL('./assets/', import.meta.url);
const size = 1024;
for (const layer of ['color','emission']) {
    const source = PNG.sync.read(await readFile(new URL(`arena-${layer}-source.png`,root)));
    for (const [name,direction] of faces) {
        const output = new PNG({width:size,height:size});
        for(let y=0;y<size;y++) for(let x=0;x<size;x++) {
            const {surface,u,v}=projectArenaRay(direction(2*(x+.5)/size-1,2*(y+.5)/size-1));
            const sx=u*source.width-.5,sy=v*source.height-.5;
            const ix=Math.floor(sx),iy=Math.floor(sy),fx=sx-ix,fy=sy-iy;
            for(let c=0;c<3;c++) {
                const pixel=(a,b)=>source.data[(Math.max(0,Math.min(source.height-1,b))*source.width+((a%source.width+source.width)%source.width))*4+c];
                output.data[(y*size+x)*4+c]=surface==='floor' ? (layer==='emission'?0:[2,3,7][c]) :
                    Math.round((pixel(ix,iy)*(1-fx)+pixel(ix+1,iy)*fx)*(1-fy)+(pixel(ix,iy+1)*(1-fx)+pixel(ix+1,iy+1)*fx)*fy);
            }
            output.data[(y*size+x)*4+3]=255;
        }
        await writeFile(new URL(`${layer}-${name}.png`,root),PNG.sync.write(output));
    }
}
console.log('12 cube faces baked from 90-degree metric views: 1024x1024, eye 1.65m, radius7m, wall12m; no illustrated floor');
