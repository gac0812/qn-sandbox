// The 3D world: a toon glass room on the desktop, Clawd inside, the key on a stand outside.
import * as THREE from 'three';
import { RoundedBoxGeometry } from 'three/addons/geometries/RoundedBoxGeometry.js';
import { toonKit, voxelFigure, clawdEyes } from './voxel.js';
import { CLAWD_GRID, CLAWD_PAL, CODEX, CODEX_PAL } from './sprites.js';

export const ROOM = { w: 190, h: 120, d: 130 };

export function createWorld(canvas, W, H) {
  const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, preserveDrawingBuffer: true });
  renderer.setPixelRatio(1); renderer.setSize(W, H, false); renderer.setClearColor(0, 0);
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(30, W / H, 1, 3000);
  const kit = toonKit();
  const { toon, outlined } = kit;
  scene.add(new THREE.AmbientLight(0xffffff, 0.75));
  const sun = new THREE.DirectionalLight(0xffffff, 1.7); sun.position.set(-120, 260, 200); scene.add(sun);

  // ---- room ----
  const room = new THREE.Group(); scene.add(room);
  const { w: BW, h: BH, d: BD } = ROOM;
  const floor = outlined(new RoundedBoxGeometry(BW + 16, 12, BD + 16, 3, 4), toon(0xF3D9C4), 1.02);
  floor.position.y = -6; room.add(floor);
  const glassMat = new THREE.MeshToonMaterial({ color: 0xBDE7F2, gradientMap: kit.ramp, transparent: true, opacity: 0.12, depthWrite: false });
  const glass = new THREE.Mesh(new RoundedBoxGeometry(BW, BH, BD, 3, 8), glassMat);
  glass.position.y = BH / 2; room.add(glass);
  const edgeMat = new THREE.MeshBasicMaterial({ color: 0x2a1a14 });
  const hx = BW / 2, hz = BD / 2;
  const corners = [[-hx, 0, -hz], [hx, 0, -hz], [hx, 0, hz], [-hx, 0, hz]];
  const edgeGroup = new THREE.Group(); room.add(edgeGroup);
  const tube = (p, q, r = 1.6) => {
    const v1 = new THREE.Vector3(...p), v2 = new THREE.Vector3(...q);
    const m = new THREE.Mesh(new THREE.CylinderGeometry(r, r, v1.distanceTo(v2), 8), edgeMat);
    m.position.copy(v1).add(v2).multiplyScalar(0.5);
    m.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), v2.clone().sub(v1).normalize());
    return m;
  };
  for (let i = 0; i < 4; i++) {
    const a = corners[i], c = corners[(i + 1) % 4];
    edgeGroup.add(tube([a[0], BH, a[2]], [c[0], BH, c[2]]));
    edgeGroup.add(tube(a, [a[0], BH, a[2]]));
  }
  const glint = new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.8 });
  for (const [x, w] of [[-60, 10], [-40, 4]]) {
    const g = new THREE.Mesh(new THREE.PlaneGeometry(w, 70), glint);
    g.position.set(x, 70, hz + 0.5); g.rotation.z = -0.5; room.add(g);
  }
  // mail slot in the right wall: the only way things pass in or out
  const slot = new THREE.Group(); room.add(slot);
  const slotFrame = outlined(new RoundedBoxGeometry(4, 34, 16, 2, 1.5), toon(0xE7D3C2), 1.12);
  const slotHole = new THREE.Mesh(new THREE.BoxGeometry(5, 26, 4), new THREE.MeshBasicMaterial({ color: 0x2a1a14 }));
  slot.add(slotFrame, slotHole); slot.position.set(hx, 46, 18);
  // cozy props
  const rug = outlined(new THREE.CylinderGeometry(52, 52, 2, 48), toon(0xE89F84), 1.03); rug.position.set(-5, 1, 15); rug.scale.z = 0.7; room.add(rug);
  const cushion = outlined(new RoundedBoxGeometry(46, 14, 30, 4, 7), toon(0x9DB1F0)); cushion.position.set(48, 8, 22); room.add(cushion);
  const lampPole = outlined(new THREE.CylinderGeometry(1.6, 1.6, 70, 8), toon(0xC9A88E), 1.4); lampPole.position.set(68, 35, -38); room.add(lampPole);
  const shade = outlined(new THREE.CylinderGeometry(9, 16, 18, 24, 1, true), toon(0xFFC96B, { side: THREE.DoubleSide, emissive: 0x553300 })); shade.position.set(68, 74, -38); room.add(shade);
  const folder = outlined(new RoundedBoxGeometry(30, 22, 6, 2, 2), toon(0x7FA7E0)); folder.position.set(-30, 13, -50); folder.rotation.y = 0.3; room.add(folder);
  const plant = outlined(new THREE.SphereGeometry(10, 16, 12), toon(0x86C79A)); plant.position.set(-74, 22, -44); room.add(plant);
  const pot = outlined(new THREE.CylinderGeometry(7, 5.5, 12, 16), toon(0xE5B07A)); pot.position.set(-74, 6, -44); room.add(pot);

  // ---- characters ----
  const clawd = voxelFigure(kit, CLAWD_GRID, CLAWD_PAL, { main: '#', skip: ['e'], vx: 4.4 });
  const eyes = clawdEyes(kit, clawd);
  const clawdRig = new THREE.Group(); clawdRig.add(clawd); room.add(clawdRig);
  const codex = voxelFigure(kit, CODEX, CODEX_PAL, { main: 's', vx: 4.2 });
  const codexRig = new THREE.Group(); codexRig.add(codex); room.add(codexRig); codexRig.visible = false;

  // handle tag (paper ticket)
  const tag = outlined(new RoundedBoxGeometry(30, 11, 1.5, 2, 1.5), toon(0xFFF3CF), 1.08);
  room.add(tag);

  // ---- key stand outside ----
  const gold = toon(0xF5BE4B, { emissive: 0x2a1500 });
  const stand = new THREE.Group(); stand.position.set(205, 0, -5); scene.add(stand);
  const base = outlined(new THREE.CylinderGeometry(22, 26, 8, 32), toon(0xE7D3C2), 1.05); base.position.y = 4; stand.add(base);
  const pole = outlined(new THREE.CylinderGeometry(2.4, 2.4, 120, 10), toon(0x9C8577), 1.35); pole.position.y = 64; stand.add(pole);
  const arm = outlined(new THREE.CylinderGeometry(2.4, 2.4, 28, 10), toon(0x9C8577), 1.35); arm.rotation.z = Math.PI / 2; arm.position.set(-12, 122, 0); stand.add(arm);
  const keyG = new THREE.Group(); keyG.position.set(-24, 120, 0); stand.add(keyG);
  const ring = outlined(new THREE.TorusGeometry(4, 1.2, 8, 20), gold, 1.15); ring.position.y = -4; keyG.add(ring);
  const keyBody = new THREE.Group(); keyG.add(keyBody);
  const bow = outlined(new THREE.TorusGeometry(11, 4.2, 12, 32), gold, 1.1); bow.position.y = -20; keyBody.add(bow);
  const shaft = outlined(new THREE.BoxGeometry(5, 40, 4), gold, 1.15); shaft.position.y = -53; keyBody.add(shaft);
  for (const [y, l] of [[-64, 9], [-72, 12]]) { const t = outlined(new THREE.BoxGeometry(l, 5, 4), gold, 1.15); t.position.set(l / 2 + 2, y, 0); keyBody.add(t); }

  // ---- architecture stack under the room (shown only in the architecture shot) ----
  // bottom → top: OS kernel isolation · nono security core · credential broker + MCP gateway · CLI + desktop panel
  const arch = new THREE.Group(); arch.visible = false; scene.add(arch);
  const slabColors = [0x9FB3C8, 0xC9694C, 0xF2C35B, 0xFFF1DE];
  const slabs = slabColors.map((c, i) => {
    const g = new THREE.Group();
    g.add(outlined(new RoundedBoxGeometry(300, 20, 230, 3, 5), toon(c), 1.03));
    g.userData.baseY = -20 - (3 - i) * 26;
    arch.add(g);
    return g;
  });
  // small props that say what each layer does
  const brickMat = toon(0xA9533A);
  for (let i = 0; i < 9; i++) { const br = new THREE.Mesh(new THREE.BoxGeometry(26, 6, 3), brickMat); br.position.set(-120 + i * 30, (i % 2) * 7 - 3, 116); slabs[1].add(br); }
  const miniKey = keyBody.clone(); miniKey.scale.setScalar(0.55); miniKey.rotation.set(0, 0, Math.PI / 2); miniKey.position.set(110, 22, 90); slabs[2].add(miniKey);
  const term = outlined(new RoundedBoxGeometry(40, 26, 4, 2, 2), toon(0x2a2f36)); term.position.set(-110, 24, 95); term.rotation.y = 0.3; slabs[3].add(term);
  const panel = outlined(new RoundedBoxGeometry(46, 30, 4, 2, 2), toon(0xFFFCF8)); panel.position.set(-62, 26, 100); panel.rotation.y = 0.2; slabs[3].add(panel);
  const panelDot = new THREE.Mesh(new THREE.BoxGeometry(10, 6, 1), toon(0xD97757)); panelDot.position.set(-70, 32, 103); slabs[3].add(panelDot);

  const v = new THREE.Vector3();
  const api = {
    THREE, renderer, scene, camera, arch, slabs, room, glass, slot, clawd, clawdRig, codexRig, eyes, tag, stand, keyG, keyBody, cushion,
    setCam(pos, look, fov = 30) { camera.position.set(...pos); camera.fov = fov; camera.updateProjectionMatrix(); camera.lookAt(...look); camera.updateMatrixWorld(); },
    // feet-anchored squash & stretch
    pose(rig, { x = 0, y = 0, z = 0, ry = 0, rz = 0, sq = 1 } = {}) {
      rig.position.set(x, y, z); rig.rotation.set(0, ry, rz);
      rig.scale.set(1 / Math.sqrt(sq), sq, 1 / Math.sqrt(sq));
    },
    project(obj, dx = 0, dy = 0, dz = 0, w = W, h = H) {
      v.set(dx, dy, dz); obj.localToWorld(v); v.project(camera);
      return [(v.x + 1) / 2 * w, (1 - v.y) / 2 * h];
    },
    rendered: false,
    clear() { renderer.setScissorTest(false); renderer.setViewport(0, 0, W, H); renderer.clear(); },
    render() { api.rendered = true; renderer.setScissorTest(false); renderer.setViewport(0, 0, W, H); camera.aspect = W / H; camera.updateProjectionMatrix(); renderer.render(scene, camera); },
    renderSplit(left, right, rightShift = 0) {
      api.rendered = true;
      // left()/right() set up the scene for each half before it is drawn
      // clear the whole canvas first: with the scissor test on, clear() only touches the scissor box
      renderer.setScissorTest(false); renderer.setViewport(0, 0, W, H); renderer.clear();
      renderer.setScissorTest(true);
      camera.aspect = (W / 2) / H; camera.updateProjectionMatrix();
      left(); renderer.setViewport(0, 0, W / 2, H); renderer.setScissor(0, 0, W / 2, H); renderer.render(scene, camera);
      right(); renderer.setViewport(W / 2 + rightShift, 0, W / 2, H); renderer.setScissor(W / 2 + rightShift, 0, W / 2, H); renderer.render(scene, camera);
      renderer.setScissorTest(false);
    },
  };
  return api;
}
