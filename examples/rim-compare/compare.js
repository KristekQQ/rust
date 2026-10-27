const comparison=document.getElementById('comparison');
const split=document.getElementById('split');
const backend=document.getElementById('backend');
const nativeImage=document.getElementById('native');
function update(){comparison.style.setProperty('--split',split.value+'%');}
split.addEventListener('input',update);
backend.addEventListener('change',()=>{const gpu=backend.value==='webgpu';nativeImage.src=gpu?'native-webgpu.png':'native-webgl.png';nativeImage.alt='Náš '+(gpu?'WebGPU':'WebGL2')+' renderer: stejná kamera a rozlišení';document.getElementById('ours-label').textContent='Náš '+(gpu?'WebGPU':'WebGL2');});
document.getElementById('flip').addEventListener('click',()=>{split.value=Number(split.value)<50?'100':'0';update();});
update();
