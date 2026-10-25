// Real WebGPU projection textures for the maintained XR pose/session simulator.
// No scene, gameplay or renderer replacement. Readback is acceptance-only.
export function installGPUProjection({width=320,height=320}={}){
 const previous=Object.getOwnPropertyDescriptor(globalThis,'XRGPUBinding'),bindings=[];
 class Projection {
  constructor(session,device){
   this.device=device;bindings.push(this);
   const schedule=session.requestAnimationFrame.bind(session);
   session.requestAnimationFrame=callback=>schedule((time,frame)=>callback(time,{
    session:frame.session,getPose:(...args)=>frame.getPose(...args),getViewerPose:space=>{
     const pose=frame.getViewerPose(space);if(!pose)return null;
     return {...pose,views:pose.views.map(view=>{
      const projectionMatrix=view.projectionMatrix.slice();
      for(let column=0;column<4;column++)projectionMatrix[column*4+2]=(projectionMatrix[column*4+2]+projectionMatrix[column*4+3])/2;
      return {...view,projectionMatrix};
     })};
    }
   }));
  }
  createProjectionLayer(options){
   this.format=options.colorFormat;
   const texture=format=>this.device.createTexture({size:[width,height,2],format,usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC});
   this.color=texture(options.colorFormat);this.depth=texture(options.depthStencilFormat);
   return this.layer={textureWidth:width,textureHeight:height,textureArrayLength:2};
  }
  getViewSubImage(layer,view){
   if(layer!==this.layer)throw Error('Unknown projection layer');
   return {colorTexture:this.color,depthStencilTexture:this.depth,viewport:{x:0,y:0,width,height},getViewDescriptor:()=>({dimension:'2d',baseArrayLayer:view.eye==='right'?1:0,arrayLayerCount:1})};
  }
  async pixels(index){
   const bytesPerRow=Math.ceil(width*4/256)*256;
   const buffer=this.device.createBuffer({size:bytesPerRow*height,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
   try{
    const encoder=this.device.createCommandEncoder();encoder.copyTextureToBuffer({texture:this.color,origin:[0,0,index]},{buffer,bytesPerRow,rowsPerImage:height},[width,height,1]);
    this.device.queue.submit([encoder.finish()]);await buffer.mapAsync(GPUMapMode.READ);
    const padded=new Uint8Array(buffer.getMappedRange()),data=new Uint8ClampedArray(width*height*4);
    for(let y=0;y<height;y++)data.set(padded.subarray(y*bytesPerRow,y*bytesPerRow+width*4),y*width*4);
    if(this.format.startsWith('bgra'))for(let i=0;i<data.length;i+=4)[data[i],data[i+2]]=[data[i+2],data[i]];
    return new ImageData(data,width,height);
   }finally{buffer.destroy();}
  }
  destroy(){this.color?.destroy();this.depth?.destroy();}
 }
 Object.defineProperty(globalThis,'XRGPUBinding',{configurable:true,writable:true,value:Projection});
 return {bindings,pixels:eye=>bindings.at(-1).pixels(eye==='right'?1:0),dispose(){for(const b of bindings)b.destroy();if(previous)Object.defineProperty(globalThis,'XRGPUBinding',previous);else delete globalThis.XRGPUBinding;}};
}
