import * as THREE from 'three';

const WIDTH = 768;
const HEIGHT = 216;
const OFFSET = new THREE.Vector3(0, -0.46, -1.35);
const clamp = (value, min, max) => Math.max(min, Math.min(max, value));
const finite = (value, fallback) => Number.isFinite(value) ? value : fallback;
const rounded = value => Math.round(value * 100) / 100;

export function createPlayerHUD(scene) {
  const canvas = document.createElement('canvas');
  canvas.width = WIDTH;
  canvas.height = HEIGHT;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('Player HUD requires a 2D canvas context.');
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.minFilter = THREE.LinearFilter;
  texture.magFilter = THREE.LinearFilter;
  texture.generateMipmaps = false;
  const material = new THREE.MeshBasicMaterial({
    map: texture, transparent: true, fog: false, toneMapped: false,
    depthTest: false, depthWrite: false,
  });
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(0.82, 0.23), material);
  mesh.name = 'player-hud';
  mesh.visible = false;
  mesh.renderOrder = 10000;
  mesh.frustumCulled = false;
  scene.add(mesh);

  let state = { hp: 100, maxHP: 100, stamina: 4, maxStamina: 4, timeScale: 1 };
  let paintedKey = '';
  let lastPaint = -Infinity;
  let refreshCount = 0;
  let disposed = false;
  const headPosition = new THREE.Vector3();
  const headQuaternion = new THREE.Quaternion();
  const localOffset = new THREE.Vector3();

  function box(x, y, w, h, radius, fill) {
    ctx.beginPath();
    ctx.roundRect(x, y, Math.max(0, w), h, radius);
    ctx.fillStyle = fill;
    ctx.fill();
  }

  function paint(now, force = false) {
    const { hp, maxHP, stamina, maxStamina } = state;
    const key = [hp, maxHP, stamina, maxStamina].map(rounded).join('/');
    if (!force && (key === paintedKey || now - lastPaint < 1000 / 12)) return;
    ctx.clearRect(0, 0, WIDTH, HEIGHT);
    box(5, 5, 758, 206, 28, 'rgba(5, 15, 24, 0.91)');
    ctx.strokeStyle = 'rgba(224, 190, 113, 0.65)';
    ctx.lineWidth = 2;
    ctx.stroke();
    box(29, 29, 5, 155, 2, '#dab978');
    ctx.textBaseline = 'alphabetic';
    ctx.textAlign = 'left';
    ctx.font = '600 25px system-ui, sans-serif';
    ctx.fillStyle = '#d7c49c';
    ctx.fillText('HP', 55, 59);
    ctx.fillText('STAMINA', 329, 59);
    ctx.font = '700 43px system-ui, sans-serif';
    ctx.fillStyle = '#fff5e4';
    ctx.fillText(String(Math.ceil(hp)), 55, 112);
    ctx.font = '500 23px system-ui, sans-serif';
    ctx.fillStyle = '#b5c4cc';
    ctx.fillText(`/ ${Math.ceil(maxHP)}`, 154, 109);
    ctx.textAlign = 'right';
    ctx.font = '700 36px system-ui, sans-serif';
    ctx.fillStyle = stamina <= 0 ? '#ffcc9b' : '#d8fff7';
    ctx.fillText(`${rounded(stamina)} / ${rounded(maxStamina)}`, 714, 110);
    box(55, 137, 220, 29, 9, '#26333d');
    if (hp > 0) {
      ctx.save();
      ctx.beginPath(); ctx.roundRect(55, 137, 220, 29, 9); ctx.clip();
      box(55, 137, 220 * hp / maxHP, 29, 0, '#f07579');
      ctx.restore();
    }
    const pipWidth = 86;
    for (let i = 0; i < 4; i++) {
      const x = 329 + i * (pipWidth + 14);
      const fraction = clamp(stamina / maxStamina * 4 - i, 0, 1);
      box(x, 137, pipWidth, 29, 8, '#263d44');
      if (fraction > 0) {
        ctx.save();
        ctx.beginPath(); ctx.roundRect(x, 137, pipWidth, 29, 8); ctx.clip();
        box(x, 137, pipWidth * fraction, 29, 0, '#6ce1cd');
        ctx.restore();
      }
    }
    ctx.textAlign = 'left';
    ctx.font = '500 17px system-ui, sans-serif';
    ctx.fillStyle = stamina <= 0 ? '#ffcc9b' : '#91aeaF';
    ctx.fillText(stamina <= 0 ? 'PRÁZDNÁ · OBNOVUJE SE' : '4 DÍLKY · OBNOVA PO PAUZE', 329, 193);
    texture.needsUpdate = true;
    paintedKey = key;
    lastPaint = now;
    refreshCount++;
  }

  paint(performance.now(), true);

  return {
    mesh,
    sync({ head, quaternion, hp, maxHP = 100, stamina, maxStamina = 4, visible = true, timeScale = 1 }) {
      if (disposed) return;
      const safeMaxHP = Math.max(0.01, finite(maxHP, 100));
      const safeMaxStamina = Math.max(0.01, finite(maxStamina, 4));
      state = {
        hp: clamp(finite(hp, state.hp), 0, safeMaxHP), maxHP: safeMaxHP,
        stamina: clamp(finite(stamina, state.stamina), 0, safeMaxStamina), maxStamina: safeMaxStamina,
        timeScale: finite(timeScale, 1),
      };
      if (head && quaternion) {
        headPosition.copy(head);
        headQuaternion.copy(quaternion).normalize();
        localOffset.copy(OFFSET).applyQuaternion(headQuaternion);
        mesh.position.copy(headPosition).add(localOffset);
        mesh.quaternion.copy(headQuaternion);
        mesh.visible = Boolean(visible);
      } else {
        mesh.visible = false;
      }
      paint(performance.now());
    },
    hide() { mesh.visible = false; },
    describe() {
      return {
        visible: mesh.visible,
        position: mesh.position.toArray(), quaternion: mesh.quaternion.toArray(),
        head: headPosition.toArray(), headQuaternion: headQuaternion.toArray(),
        offset: OFFSET.toArray(), size: [0.82, 0.23],
        ...state, refreshCount,
      };
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      mesh.removeFromParent();
      mesh.geometry.dispose(); material.dispose(); texture.dispose();
    },
  };
}
