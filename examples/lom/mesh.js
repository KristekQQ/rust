function bindLomMesh(gl,program){
 // Adjacent triangles share identical moving ring vertices across the seam.
 const vertices=[];
 function triangle(a,b,c){
  for(let facet=0;facet<3;facet++){
   for(let i=0;i<3;i++)vertices.push(...a,...b,...c,Number(i===0),Number(i===1),Number(i===2),facet);
  }
 }
 for(let z=0;z<44;z++)for(let j=0;j<24;j++){
  const a=[j,z],b=[j+1,z],c=[j,z+1],d=[j+1,z+1];
  triangle(a,b,c);triangle(b,d,c);
 }
 const b=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,b);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array(vertices),gl.STATIC_DRAW);
 for(const [name,count,offset] of [['cornerA',2,0],['cornerB',2,2],['cornerC',2,4],['bary',3,6],['facet',1,9]]){
  const loc=gl.getAttribLocation(program,name);gl.enableVertexAttribArray(loc);gl.vertexAttribPointer(loc,count,gl.FLOAT,false,40,offset*4);
 }
 return vertices.length/10;
}
