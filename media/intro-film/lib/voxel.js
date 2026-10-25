// Toon-shaded voxel figures from palette grids: one extruded silhouette in the main colour,
// a dilated back-face hull for the ink line, and thin front plates for the other colours.
import * as THREE from 'three';

export function toonKit() {
  const ramp = new THREE.DataTexture(new Uint8Array([90, 90, 90, 255, 175, 175, 175, 255, 255, 255, 255, 255]), 3, 1);
  ramp.minFilter = ramp.magFilter = THREE.NearestFilter; ramp.needsUpdate = true;
  const toon = (c, extra = {}) => new THREE.MeshToonMaterial({ color: c, gradientMap: ramp, ...extra });
  const inkMat = new THREE.MeshBasicMaterial({ color: 0x2a1a14, side: THREE.BackSide });
  const outlined = (geo, mat, s = 1.06) => {
    const g = new THREE.Group();
    g.add(new THREE.Mesh(geo, mat));
    const o = new THREE.Mesh(geo, inkMat); o.scale.setScalar(s); g.add(o);
    return g;
  };
  return { ramp, toon, inkMat, outlined };
}

function traceShape(isOn, cols, rows, cell, ox, oy) {
  // Directed boundary edges with the filled cell on the left, chained into loops. Where two
  // cells touch only at a corner a vertex has two exits; taking the left turn keeps the loops
  // simple (no self-touching polygons, which the triangulator rejects).
  const out = new Map();
  const add = (c0, r0, c1, r1) => { const k = c0 * 4096 + r0; (out.get(k) || out.set(k, []).get(k)).push([c1, r1]); };
  for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
    if (!isOn(r, c)) continue;
    if (!isOn(r - 1, c)) add(c + 1, r, c, r);
    if (!isOn(r + 1, c)) add(c, r + 1, c + 1, r + 1);
    if (!isOn(r, c - 1)) add(c, r, c, r + 1);
    if (!isOn(r, c + 1)) add(c + 1, r + 1, c + 1, r);
  }
  const shapes = [];
  for (const [k0, list] of out) {
    while (list.length) {
      let c = Math.floor(k0 / 4096), r = k0 % 4096;
      const shape = new THREE.Shape();
      shape.moveTo(ox + c * cell, oy - r * cell);
      let [nc, nr] = list.pop();
      let dx = nc - c, dy = nr - r;
      c = nc; r = nr;
      const startK = k0;
      let guard = 0;
      while (c * 4096 + r !== startK && guard++ < 1e6) {
        shape.lineTo(ox + c * cell, oy - r * cell);
        const exits = out.get(c * 4096 + r);
        let pick = 0;
        if (exits.length > 1) {
          const lx = dy, ly = -dx; // left turn in y-down grid coordinates
          const i = exits.findIndex(([ec, er]) => ec - c === lx && er - r === ly);
          pick = i >= 0 ? i : 0;
        }
        [nc, nr] = exits.splice(pick, 1)[0];
        dx = nc - c; dy = nr - r; c = nc; r = nr;
      }
      shapes.push(shape);
    }
  }
  return shapes;
}

// grid: rows of palette keys (null = empty). main: keys extruded as the body.
export function voxelFigure(kit, grid, pal, o = {}) {
  const VX = o.vx || 4.4, depth = (o.depth || 4) * VX;
  const rows = grid.length, cols = grid[0].length;
  const on = (r, c) => r >= 0 && r < rows && c >= 0 && c < cols && grid[r][c] != null;
  const ox = -cols / 2 * VX, oy = rows * VX;
  const fig = new THREE.Group();
  const body = new THREE.ExtrudeGeometry(traceShape(on, cols, rows, VX, ox, oy), { depth, bevelEnabled: false });
  body.translate(0, 0, -depth / 2);
  fig.add(new THREE.Mesh(body, kit.toon(new THREE.Color(pal[o.main]))));
  fig.userData.bodyMat = fig.children[0].material;
  const SUB = 4;
  const dil = (r, c) => {
    for (let dr = -1; dr <= 1; dr++) for (let dc = -1; dc <= 1; dc++) {
      const rr = r + dr - 1, cc = c + dc - 1;
      if (rr >= 0 && cc >= 0 && on(Math.floor(rr / SUB), Math.floor(cc / SUB))) return true;
    }
    return false;
  };
  const hull = new THREE.ExtrudeGeometry(traceShape(dil, cols * SUB + 2, rows * SUB + 2, VX / SUB, ox - VX / SUB, oy + VX / SUB), { depth: depth * 1.12, bevelEnabled: false });
  hull.translate(0, 0, -depth * 0.56);
  fig.add(new THREE.Mesh(hull, kit.inkMat));
  // front plates for every non-main colour (eyes, screen, belly…), on both faces
  const plate = new THREE.BoxGeometry(VX, VX, VX * 0.3);
  const byKey = {};
  for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
    const k = grid[r][c]; if (k == null || k === o.main || (o.skip && o.skip.includes(k))) continue;
    (byKey[k] ||= []).push([r, c]);
  }
  const m = new THREE.Matrix4();
  for (const [k, cells] of Object.entries(byKey)) {
    const inst = new THREE.InstancedMesh(plate, kit.toon(new THREE.Color(pal[k])), cells.length * 2);
    cells.forEach(([r, c], i) => {
      const x = ox + (c + 0.5) * VX, y = oy - (r + 0.5) * VX;
      m.makeTranslation(x, y, depth / 2 + VX * 0.1); inst.setMatrixAt(i * 2, m);
      m.makeTranslation(x, y, -depth / 2 - VX * 0.1); inst.setMatrixAt(i * 2 + 1, m);
    });
    fig.add(inst);
  }
  Object.assign(fig.userData, { w: cols * VX, h: rows * VX, depth, VX, ox, oy });
  return fig;
}

// Clawd's eyes as a separate rig so the 3D figure can act: open / happy / wide / shut / look-*.
export function clawdEyes(kit, fig) {
  const { VX, ox, oy, depth } = fig.userData;
  const mat = kit.toon(0x1E1410);
  const g = new THREE.Group();
  const eyes = [5, 12].map((c) => {
    const e = new THREE.Group();
    e.position.set(ox + (c + 0.5) * VX, oy - 3 * VX, depth / 2 + VX * 0.12);
    const open = new THREE.Mesh(new THREE.BoxGeometry(VX, VX * 2, VX * 0.3), mat);
    const lid = new THREE.Mesh(new THREE.BoxGeometry(VX * 1.6, VX * 0.5, VX * 0.3), mat);
    const arcL = new THREE.Mesh(new THREE.BoxGeometry(VX * 0.6, VX * 0.6, VX * 0.3), mat);
    const arcM = new THREE.Mesh(new THREE.BoxGeometry(VX, VX * 0.6, VX * 0.3), mat);
    const arcR = arcL.clone();
    arcL.position.set(-VX * 0.75, -VX * 0.1, 0); arcM.position.set(0, VX * 0.45, 0); arcR.position.set(VX * 0.75, -VX * 0.1, 0);
    const happy = new THREE.Group(); happy.add(arcL, arcM, arcR);
    lid.position.y = -VX * 0.55;
    e.add(open, lid, happy);
    return { e, open, lid, happy };
  });
  eyes.forEach(({ e }) => g.add(e));
  fig.add(g);
  const set = (name, k = 1) => {
    for (const { open, lid, happy } of eyes) {
      open.visible = ['open', 'wide', 'look-l', 'look-r', 'look-up'].includes(name);
      lid.visible = name === 'shut';
      happy.visible = name === 'happy';
      open.scale.set(name === 'wide' ? 1.5 : 1, name === 'wide' ? 1.3 : name === 'look-up' ? 0.8 : 1, 1);
      open.position.set(name === 'look-l' ? -VX * 0.5 : name === 'look-r' ? VX * 0.5 : 0, name === 'look-up' ? VX * 0.3 : 0, 0);
      if (name === 'blink') { open.visible = true; open.scale.y = 0.15; }
    }
  };
  set('open');
  return { set };
}
