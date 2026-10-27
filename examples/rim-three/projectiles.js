import * as THREE from 'three';

const CAPACITY = 32;
const ENEMY_SPEED = 3.2;
const RETURN_SPEED = 5.5;
const TTL = 8;
const BLADE_RADIUS = 0.075;
const EPS = 1e-7;
const validVector = v => v && Number.isFinite(v.x) && Number.isFinite(v.y) && Number.isFinite(v.z);

// Earliest intersection of a swept point with a sphere, including initial overlap.
function sphereTime(from, to, center, radius) {
  const d = to.clone().sub(from), m = from.clone().sub(center);
  const c = m.lengthSq() - radius * radius;
  if (c <= 0) return 0;
  const a = d.lengthSq(), b = m.dot(d), disc = b * b - a * c;
  if (a < EPS || b >= 0 || disc < 0) return null;
  const t = (-b - Math.sqrt(disc)) / a;
  return t >= 0 && t <= 1 ? t : null;
}

function pointSegmentDistance(point, a, b) {
  const ab = b.clone().sub(a);
  const t = ab.lengthSq() > EPS ? THREE.MathUtils.clamp(point.clone().sub(a).dot(ab) / ab.lengthSq(), 0, 1) : 0;
  return point.distanceTo(a.clone().addScaledVector(ab, t));
}

// Conservative advancement on a linearly swept capsule. The Lipschitz speed
// bound includes BOTH moving endpoints, so a stationary bolt can be intercepted.
function bladeTime(from, to, blade, frameStart, frameEnd) {
  const oldA = blade.previousA || blade.a, oldB = blade.previousB || blade.b;
  const a0 = oldA.clone().lerp(blade.a, frameStart);
  const b0 = oldB.clone().lerp(blade.b, frameStart);
  const a1 = oldA.clone().lerp(blade.a, frameEnd);
  const b1 = oldB.clone().lerp(blade.b, frameEnd);
  const speed = from.distanceTo(to) + Math.max(a0.distanceTo(a1), b0.distanceTo(b1));
  let t = 0;
  for (let i = 0; i < 256; i++) {
    const gap = pointSegmentDistance(from.clone().lerp(to, t), a0.clone().lerp(a1, t), b0.clone().lerp(b1, t)) - BLADE_RADIUS;
    if (gap <= 0.00001) return t;
    if (speed < EPS) return null;
    const advance = gap / speed;
    if (t + advance > 1) return null;
    t += advance;
  }
  // Rare near-tangent convergence: conservatively block instead of tunnelling.
  return t;
}

export function createProjectileSystem({ scene, castWorld, onEnemyHit = () => {}, onPlayerHit = () => {} }) {
  const geometry = new THREE.SphereGeometry(0.055, 8, 6);
  const hostileMaterial = new THREE.MeshBasicMaterial({ color: 0xff6b18, toneMapped: false });
  const friendlyMaterial = new THREE.MeshBasicMaterial({ color: 0x28edff, toneMapped: false });
  const pool = Array.from({ length: CAPACITY }, () => {
    const mesh = new THREE.Mesh(geometry, hostileMaterial);
    mesh.visible = false;
    scene.add(mesh);
    return { mesh, active: false, position: mesh.position, velocity: new THREE.Vector3(), id: 0, ownerId: 0, friendly: false, life: 0, grace: 0 };
  });
  const stats = { fired: 0, reflected: 0, hitEnemies: 0, hitPlayer: 0, blockedWorld: 0 };
  let nextId = 1, disposed = false;
  const remove = bolt => { bolt.active = false; bolt.mesh.visible = false; };

  function shoot(origin, target, ownerId) {
    if (disposed || !validVector(origin) || !validVector(target) || !Number.isSafeInteger(ownerId)) return false;
    const direction = new THREE.Vector3().subVectors(target, origin);
    if (!Number.isFinite(direction.lengthSq()) || direction.lengthSq() < EPS) return false;
    const bolt = pool.find(item => !item.active);
    if (!bolt) return false;
    Object.assign(bolt, { active: true, id: nextId++, ownerId, friendly: false, life: TTL, grace: 0 });
    bolt.position.copy(origin);
    bolt.velocity.copy(direction).normalize().multiplyScalar(ENEMY_SPEED);
    bolt.mesh.material = hostileMaterial;
    bolt.mesh.visible = true;
    stats.fired++;
    return true;
  }

  function update(worldDt, frame) {
    // Validate before touching pool, counters, lifetime or callbacks.
    if (disposed || !Number.isFinite(worldDt) || worldDt < 0 || !frame) return false;
    const { blades, targets, player } = frame;
    if (!Array.isArray(blades) || !Array.isArray(targets) || !player || !validVector(player.head) || !Number.isFinite(player.radius) || player.radius < 0 ||
      blades.some(b => !b || !validVector(b.a) || !validVector(b.b) || (b.previousA !== undefined && !validVector(b.previousA)) || (b.previousB !== undefined && !validVector(b.previousB))) ||
      targets.some(t => !t || !Number.isSafeInteger(t.id) || !validVector(t.position) || !Number.isFinite(t.radius) || t.radius < 0 || typeof t.active !== 'boolean')) return false;
    if (worldDt === 0) return true;
    const bodyCenter = player.head.clone().add(new THREE.Vector3(0, -0.25, 0));
    for (const bolt of pool) {
      if (!bolt.active) continue;
      let remaining = Math.min(worldDt, bolt.life), elapsed = 0;
      while (bolt.active && remaining > EPS) {
        // Split at grace expiry so even very large frames remain continuous.
        const duration = bolt.grace > EPS ? Math.min(remaining, bolt.grace) : remaining;
        const from = bolt.position.clone(), to = from.clone().addScaledVector(bolt.velocity, duration);
        let nearest = null;
        const consider = (fraction, type, target = null) => {
          if (fraction !== null && Number.isFinite(fraction) && fraction >= 0 && fraction <= 1 && (!nearest || fraction < nearest.fraction - EPS)) nearest = { fraction, type, target };
        };
        const wall = castWorld(from, to);
        if (wall) consider(wall.fraction, 'wall'); // Walls win ties.
        if (!bolt.friendly && bolt.grace <= EPS) for (const blade of blades) consider(bladeTime(from, to, blade, elapsed / worldDt, (elapsed + duration) / worldDt), 'blade');
        if (bolt.friendly) {
          for (const target of targets) if (target.active) consider(sphereTime(from, to, target.position, target.radius + 0.055), 'enemy', target);
        } else consider(sphereTime(from, to, bodyCenter, player.radius + 0.055), 'player');
        const used = duration * (nearest ? nearest.fraction : 1);
        bolt.position.lerpVectors(from, to, nearest ? nearest.fraction : 1);
        bolt.life -= used;
        bolt.grace = Math.max(0, bolt.grace - used);
        elapsed += used;
        remaining -= used;
        if (!nearest) continue;
        if (nearest.type === 'blade') {
          const live = targets.filter(t => t.active);
          const source = live.find(t => t.id === bolt.ownerId) || live.reduce((best, t) => !best || t.position.distanceToSquared(bolt.position) < best.position.distanceToSquared(bolt.position) ? t : best, null);
          const direction = source ? source.position.clone().sub(bolt.position) : bolt.velocity.clone().negate();
          if (direction.lengthSq() < EPS) direction.copy(bolt.velocity).negate();
          bolt.velocity.copy(direction).normalize().multiplyScalar(RETURN_SPEED);
          bolt.friendly = true;
          bolt.grace = 0.12;
          bolt.mesh.material = friendlyMaterial;
          stats.reflected++;
        } else {
          remove(bolt); // Remove before callback, including callback-triggered shooting.
          if (nearest.type === 'wall') stats.blockedWorld++;
          if (nearest.type === 'enemy') { stats.hitEnemies++; onEnemyHit(nearest.target.id); }
          if (nearest.type === 'player') { stats.hitPlayer++; onPlayerHit(); }
        }
      }
      if (bolt.life <= EPS) remove(bolt);
    }
    return true;
  }

  // Wave reset: retain the fixed pool, shared GPU assets and cumulative stats.
  function clear() {
    for (const bolt of pool) remove(bolt);
  }
  // Terminal cleanup: release the shared GPU assets exactly once.
  function dispose() {
    if (disposed) return;
    clear();
    for (const bolt of pool) scene.remove(bolt.mesh);
    geometry.dispose(); hostileMaterial.dispose(); friendlyMaterial.dispose();
    disposed = true;
  }
  function describe() {
    return { ...stats, capacity: CAPACITY, active: pool.filter(b => b.active).map(b => ({ id: b.id, ownerId: b.ownerId, friendly: b.friendly, position: b.position.toArray(), velocity: b.velocity.toArray(), life: b.life })) };
  }
  return { shoot, update, clear, dispose, describe };
}
