// ══════════════════════════════════════════════════════════════════
// Elore 3D Relationship Graph — three.js + 3d-force-graph
// ══════════════════════════════════════════════════════════════════

const API = '';

// ── State ────────────────────────────────────────────────────────

let graphData = null;
let graph3d = null;
let selectedNode = null;
let activeEdgeTypes = new Set([
  'relationship', 'location', 'membership', 'rivalry', 'connection', 'goal_conflict'
]);

// Colors
const NODE_COLOR = {
  character: '#4a7fff',
  location:  '#2ecc71',
  faction:   '#e67e22',
};

const EDGE_COLOR = {
  relationship:  '#6a8fff',
  location:      '#2ecc71',
  membership:    '#e67e22',
  rivalry:       '#e74c3c',
  connection:    '#2ecc7180',
  goal_conflict: '#ff4444',
};

// ── Init ─────────────────────────────────────────────────────────

async function init() {
  const phases = await fetchJson('/api/phases');
  const select = document.getElementById('phase-select');

  if (phases.plan.length === 0 && Object.keys(phases.phases).length === 0) {
    select.innerHTML = '<option disabled>No phases found</option>';
    return;
  }

  const phaseIds = phases.plan.length > 0
    ? phases.plan
    : Object.keys(phases.phases).sort();

  for (const id of phaseIds) {
    const opt = document.createElement('option');
    opt.value = id;
    opt.textContent = id;
    if (id === phases.current) opt.selected = true;
    select.appendChild(opt);
  }

  select.addEventListener('change', () => loadGraph(select.value));

  // Edge filter checkboxes
  document.querySelectorAll('#edge-filters input').forEach(cb => {
    cb.addEventListener('change', () => {
      activeEdgeTypes = new Set();
      document.querySelectorAll('#edge-filters input:checked').forEach(c => {
        activeEdgeTypes.add(c.value);
      });
      if (graph3d) updateVisibility();
    });
  });

  // Secret view radio
  document.querySelectorAll('#secret-view input').forEach(r => {
    r.addEventListener('change', () => {
      if (selectedNode) showDetail(selectedNode);
    });
  });

  // Init 3D graph
  const container = document.getElementById('graph-container');
  graph3d = ForceGraph3D()(container)
    .backgroundColor('#0a0a0f')
    .showNavInfo(false)
    // Nodes
    .nodeThreeObject(makeNodeObject)
    .nodeThreeObjectExtend(false)
    // Links
    .linkWidth(d => activeEdgeTypes.has(d.type) ? 1.2 : 0)
    .linkOpacity(0.6)
    .linkColor(d => activeEdgeTypes.has(d.type) ? (EDGE_COLOR[d.type] || '#555') : 'transparent')
    .linkDirectionalParticles(d => d.type === 'relationship' ? 2 : 0)
    .linkDirectionalParticleWidth(1.5)
    .linkDirectionalParticleColor(d => EDGE_COLOR[d.type] || '#555')
    // Interaction
    .onNodeClick(onNodeClick)
    .onBackgroundClick(onBackgroundClick)
    .onNodeHover(onNodeHover);

  // Load initial graph
  const initialPhase = phases.current || phaseIds[0];
  select.value = initialPhase;
  await loadGraph(initialPhase);
}

// ── Data fetching ────────────────────────────────────────────────

async function fetchJson(url) {
  const res = await fetch(API + url);
  if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
  return res.json();
}

async function loadGraph(phase) {
  try {
    graphData = await fetchJson(`/api/graph/${encodeURIComponent(phase)}`);
    renderGraph(graphData);
  } catch (e) {
    console.error('Failed to load graph:', e);
  }
}

// ── 3D Rendering ─────────────────────────────────────────────────

function renderGraph(data) {
  const nodes = data.nodes.map(d => ({ ...d }));
  const nodeIds = new Set(nodes.map(n => n.id));
  const links = data.edges
    .filter(e => nodeIds.has(e.source) && nodeIds.has(e.target))
    .map(d => ({ ...d }));

  graph3d.graphData({ nodes, links });

  // Force tuning (must be after graphData so the engine exists)
  const charge = graph3d.d3Force('charge');
  if (charge) charge.strength(-120);
  const link = graph3d.d3Force('link');
  if (link) link.distance(80);

  // Short warm-up then settle
  setTimeout(() => graph3d.zoomToFit(400, 60), 1500);
}

function makeNodeObject(node) {
  const group = new THREE.Group();

  // Sphere
  const r = node.type === 'character' ? 6 : node.type === 'faction' ? 5 : 4;
  const color = NODE_COLOR[node.type] || '#888';

  let mesh;
  if (node.type === 'location') {
    // Box for locations
    const geo = new THREE.BoxGeometry(r * 1.6, r * 1.6, r * 1.6);
    const mat = new THREE.MeshLambertMaterial({ color, transparent: true, opacity: 0.85 });
    mesh = new THREE.Mesh(geo, mat);
  } else if (node.type === 'faction') {
    // Octahedron for factions
    const geo = new THREE.OctahedronGeometry(r);
    const mat = new THREE.MeshLambertMaterial({ color, transparent: true, opacity: 0.85 });
    mesh = new THREE.Mesh(geo, mat);
  } else {
    // Sphere for characters
    const geo = new THREE.SphereGeometry(r, 16, 12);
    const mat = new THREE.MeshLambertMaterial({ color, transparent: true, opacity: 0.85 });
    mesh = new THREE.Mesh(geo, mat);
  }
  group.add(mesh);

  // Glow ring for characters
  if (node.type === 'character') {
    const ringGeo = new THREE.RingGeometry(r + 1, r + 2, 32);
    const ringMat = new THREE.MeshBasicMaterial({
      color, transparent: true, opacity: 0.3, side: THREE.DoubleSide
    });
    const ring = new THREE.Mesh(ringGeo, ringMat);
    group.add(ring);
  }

  // Text label
  const sprite = new SpriteText(node.name, 3.5, '#e0e0e0');
  sprite.fontFace = 'PingFang SC, SF Pro, system-ui, sans-serif';
  sprite.backgroundColor = 'rgba(0,0,0,0.5)';
  sprite.borderRadius = 2;
  sprite.padding = [1, 3];
  sprite.position.y = -(r + 6);
  group.add(sprite);

  // Secret indicator dot
  if (graphData && graphData.secrets.some(s => s.known_by.includes(node.id))) {
    const dotGeo = new THREE.SphereGeometry(1.5, 8, 6);
    const dotMat = new THREE.MeshBasicMaterial({ color: '#a855f7' });
    const dot = new THREE.Mesh(dotGeo, dotMat);
    dot.position.set(r - 1, r - 1, 0);
    group.add(dot);
  }

  return group;
}

// ── Visibility update (edge filter) ──────────────────────────────

function updateVisibility() {
  // Re-trigger link styling by refreshing graphData
  graph3d
    .linkWidth(d => activeEdgeTypes.has(d.type) ? 1.2 : 0)
    .linkColor(d => activeEdgeTypes.has(d.type) ? (EDGE_COLOR[d.type] || '#555') : 'transparent')
    .linkDirectionalParticles(d => {
      if (!activeEdgeTypes.has(d.type)) return 0;
      return d.type === 'relationship' ? 2 : 0;
    });
}

// ── Interactions ─────────────────────────────────────────────────

function onNodeClick(node) {
  if (!node) return;
  selectedNode = node;
  showDetail(node);

  // Focus camera
  const dist = 120;
  const pos = node;
  graph3d.cameraPosition(
    { x: pos.x + dist, y: pos.y + dist * 0.3, z: pos.z + dist },
    { x: pos.x, y: pos.y, z: pos.z },
    800
  );
}

function onBackgroundClick() {
  selectedNode = null;
  clearDetail();
}

let hoveredNode = null;
function onNodeHover(node) {
  const container = document.getElementById('graph-container');
  container.style.cursor = node ? 'pointer' : 'default';
  hoveredNode = node || null;
}

// ── Detail panel ─────────────────────────────────────────────────

function showDetail(d) {
  const title = document.getElementById('detail-title');
  const body = document.getElementById('detail-body');

  const typeLabel = { character: 'Character', location: 'Location', faction: 'Faction' };
  title.textContent = `${d.name} (${typeLabel[d.type]})`;

  let html = '';

  if (d.traits && d.traits.length > 0) {
    html += `<h3>Traits</h3><div>${d.traits.map(t => `<span class="tag">${esc(t)}</span>`).join('')}</div>`;
  }

  if (d.beliefs && d.beliefs.length > 0) {
    html += `<h3>Beliefs</h3><ul>${d.beliefs.map(b => `<li>${esc(b)}</li>`).join('')}</ul>`;
  }

  if (d.desires && d.desires.length > 0) {
    html += `<h3>Desires</h3><ul>${d.desires.map(b => `<li>${esc(b)}</li>`).join('')}</ul>`;
  }

  if (d.location) {
    const locNode = graphData.nodes.find(n => n.id === d.location);
    html += `<h3>Location</h3><div>${esc(locNode ? locNode.name : d.location)}</div>`;
  }

  if (d.properties && d.properties.length > 0) {
    html += `<h3>Properties</h3><div>${d.properties.map(p => `<span class="tag">${esc(p)}</span>`).join('')}</div>`;
  }

  if (d.members && d.members.length > 0) {
    html += `<h3>Members</h3><div>${d.members.map(m => {
      const mn = graphData.nodes.find(n => n.id === m);
      return `<span class="tag">${esc(mn ? mn.name : m)}</span>`;
    }).join('')}</div>`;
  }

  if (d.goals && d.goals.length > 0) {
    html += `<h3>Goals</h3>`;
    for (const g of d.goals) {
      html += `<div class="tag" style="display:block;margin:4px 0;"><b>${esc(g.id)}</b> [${g.status}]: ${esc(g.want)}</div>`;
    }
  }

  // Relationships
  const rels = graphData.edges.filter(e => {
    const sid = typeof e.source === 'object' ? e.source.id : e.source;
    return e.type === 'relationship' && sid === d.id;
  });
  if (rels.length > 0) {
    html += `<h3>Relationships</h3>`;
    for (const r of rels) {
      const tid = typeof r.target === 'object' ? r.target.id : r.target;
      const tn = graphData.nodes.find(n => n.id === tid);
      html += `<div class="tag" style="display:block;margin:2px 0;">${esc(r.label || '')} → ${esc(tn ? tn.name : tid)}</div>`;
    }
  }

  // Secrets
  const secretMode = document.querySelector('#secret-view input:checked').value;
  const relevantSecrets = graphData.secrets.filter(s => {
    if (secretMode === 'reader' && !s.revealed_to_reader) return false;
    return s.known_by.includes(d.id);
  });

  if (relevantSecrets.length > 0) {
    html += `<h3>Secrets</h3>`;
    for (const s of relevantSecrets) {
      html += `<div class="secret-card">
        <div class="label">${esc(s.id)}${s.dramatic_function ? ' / ' + esc(s.dramatic_function) : ''}</div>
        <div>${esc(s.content)}</div>
        <div style="color:#888;font-size:10px;margin-top:4px;">Known by: ${s.known_by.map(esc).join(', ')}</div>
      </div>`;
    }
  }

  body.innerHTML = html;
}

function clearDetail() {
  document.getElementById('detail-title').textContent = 'Select a node';
  document.getElementById('detail-body').innerHTML = '';
}

function esc(s) {
  const el = document.createElement('span');
  el.textContent = s;
  return el.innerHTML;
}

// ── Boot ─────────────────────────────────────────────────────────

init().catch(e => {
  document.body.innerHTML = `<pre style="color:red;padding:20px;">Init failed:\n${e}\n\nCheck:\n1. Is elore-server running?\n2. Open F12 console for details.</pre>`;
  console.error(e);
});
