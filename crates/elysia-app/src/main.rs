//! Elysia — first demo: a 100x100 world with ground and one man.
//!
//! Wires the layers together and serves the browser client. The board holds state
//! and sends text; the browser draws the characters on a canvas (P4, A5 `ascii`,
//! B7 `canvas2d`). Nothing here is gameplay beyond walking (C3 `minimal`, whose
//! rationale says even minimal is too much yet — world and tooling first).

use elysia_net::{Request, Response, serve};
use elysia_world::{MAPH, MAPW, Man, World};
use std::fmt::Write as _;

/// The client. Kept as one string so the board serves it from memory — no asset
/// pipeline, no second request, nothing to go stale on disk.
const PAGE: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><title>elysia — 100x100</title>
<style>
  html,body{margin:0;height:100%;background:#0b0d10;overflow:hidden}
  #wrap{position:fixed;inset:0;display:grid;place-items:center}
  canvas{display:block;image-rendering:pixelated;cursor:crosshair}
  #hud{position:fixed;left:10px;bottom:8px;color:#7f8c98;
       font:12px ui-monospace,Menlo,Consolas,monospace;white-space:pre}
  #hud b{color:#cfd8e3;font-weight:600}
</style></head>
<body>
<div id="wrap"><canvas id="c"></canvas></div>
<div id="hud">loading…</div>
<script>
const CW = 11, CH = 15;             // cell size in px
const cv = document.getElementById('c'), g = cv.getContext('2d');
const hud = document.getElementById('hud');
const COL = {'.':'#3f6f4a', ',':'#8a6f4a', '^':'#7b7b84', '~':'#2f5f8f', '@':'#ffcc33'};
let map = null, man = null, seed = 0, hash = '', rtt = 0;

function resize() {
  cv.width  = Math.floor(innerWidth  / CW) * CW;
  cv.height = Math.floor(innerHeight / CH) * CH;
  draw();
}
addEventListener('resize', resize);

function draw() {
  if (!map || !man) return;
  const cols = cv.width / CW, rows = cv.height / CH;
  // camera centred on the man, clamped to the map
  let ox = man.x - (cols >> 1), oy = man.y - (rows >> 1);
  ox = Math.max(0, Math.min(ox, map[0].length - cols));
  oy = Math.max(0, Math.min(oy, map.length - rows));

  g.fillStyle = '#0b0d10'; g.fillRect(0, 0, cv.width, cv.height);
  g.font = `${CH - 3}px ui-monospace, Menlo, Consolas, monospace`;
  g.textBaseline = 'top';
  for (let r = 0; r < rows; r++) {
    const y = oy + r;
    if (y < 0 || y >= map.length) continue;
    const line = map[y];
    for (let c = 0; c < cols; c++) {
      const x = ox + c;
      if (x < 0 || x >= line.length) continue;
      const ch = (x === man.x && y === man.y) ? '@' : line[x];
      g.fillStyle = COL[ch] || '#888';
      g.fillText(ch, c * CW, r * CH);
    }
  }
  hud.innerHTML =
    `seed <b>${seed}</b>  pos <b>${man.x},${man.y}</b>  ` +
    `hash <b>${hash}</b>  rtt <b>${rtt.toFixed(1)}ms</b>\n` +
    `wasd / arrows to walk`;
}

function apply(j) {
  map = j.rows; man = j.man; seed = j.seed; hash = j.hash;
  draw();
}

async function refresh() {
  const t0 = performance.now();
  try {
    const j = await (await fetch('/state', {cache:'no-store'})).json();
    rtt = performance.now() - t0;
    apply(j);
  } catch (e) { hud.textContent = 'server unreachable: ' + e; }
}

async function move(d) {
  const t0 = performance.now();
  try {
    const j = await (await fetch('/move?d=' + d, {method:'POST'})).json();
    rtt = performance.now() - t0;
    apply(j);
  } catch (e) { /* transient; the poll loop will recover */ }
}

const KEYS = {
  ArrowUp:'n', ArrowDown:'s', ArrowLeft:'w', ArrowRight:'e',
  w:'n', s:'s', a:'w', d:'e', W:'n', S:'s', A:'w', D:'e'
};
addEventListener('keydown', ev => {
  const d = KEYS[ev.key];
  if (d) { ev.preventDefault(); move(d); }
});

resize();
refresh();
setInterval(refresh, 250);
</script>
</body></html>"##;

/// Serialise world + man for the client. `hash` is a string on purpose: a u64
/// exceeds JavaScript's safe integer range and would lose precision as a number.
fn state_json(world: &mut World, man: Man) -> String {
    // state_hash, not hash(): the fingerprint must cover the entity too, or two
    // different game states report the same number (L01).
    let hash = world.state_hash(&man);
    let rows = world.rows();
    let mut s = String::with_capacity(MAPW as usize * MAPH as usize + 512);
    let _ = write!(
        s,
        "{{\"seed\":{},\"w\":{MAPW},\"h\":{MAPH},\"man\":{{\"x\":{},\"y\":{}}},\"hash\":\"{hash:016x}\",\"rows\":[",
        world.seed, man.x, man.y
    );
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // Rows carry only terrain characters, so JSON escaping is unnecessary —
        // asserted rather than assumed, since a stray quote would corrupt it.
        debug_assert!(row.chars().all(|c| ".,^~".contains(c)));
        let _ = write!(s, "\"{row}\"");
    }
    s.push_str("]}");
    s
}

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8125".to_string());
    // A seed on the command line makes a run reproducible by hand (C6: the world
    // is generated from a seed, so the same seed is the same world).
    let seed: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20261111);

    let mut world = World::new(seed);
    let mut man = Man::spawn(&mut world);
    eprintln!("seed {seed}, man spawned at {},{}", man.x, man.y);

    let result = serve(&addr, move |req: &Request| -> Response {
        match (req.method.as_str(), req.path.as_str()) {
            ("GET", "/") => Response::html(PAGE.to_string()),
            ("GET", "/state") => Response::json(state_json(&mut world, man)),
            ("POST", "/move") => {
                let d = req.param("d").unwrap_or("");
                let (dx, dy) = match d {
                    "n" => (0, -1),
                    "s" => (0, 1),
                    "w" => (-1, 0),
                    "e" => (1, 0),
                    _ => return Response::bad_request("d must be one of n,s,e,w"),
                };
                man.step(&mut world, dx, dy);
                Response::json(state_json(&mut world, man))
            }
            // D8 metrics + E1 semver3: the version has to be answerable at
            // runtime, or "which build is on the board?" is answerable only by
            // hashing the binary (L08). The hash makes a silent stall visible.
            ("GET", "/health") => Response::json(format!(
                "{{\"ok\":true,\"version\":\"{}\",\"seed\":{seed},\"man\":[{},{}],\"hash\":\"{:016x}\"}}",
                env!("CARGO_PKG_VERSION"),
                man.x,
                man.y,
                world.state_hash(&man)
            )),
            _ => Response::not_found(),
        }
    });
    if let Err(e) = result {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
