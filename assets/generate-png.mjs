// Exports SuiCash asset SVGs to PNG.
//
//   npm --prefix assets install
//   npm --prefix assets run generate-png [-- --scale=2]
//
// Writes a *.png next to each *.svg under assets/ (SVGs are the source of truth; PNGs are
// generated and not tracked in git). Rasterized with resvg (Rust, no native deps).
//
// The logo SVG's <text> is set in the REM variable font (wght 605), so REM[wght].ttf is
// fetched from the Google Fonts repository and passed to resvg.
// Network is needed only on the first run; the font is cached in TMPDIR (same as generate.py).
// To use a local font instead, set its path in SUICASH_REM_TTF.
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REM_URL =
  "https://raw.githubusercontent.com/google/fonts/main/ofl/rem/REM%5Bwght%5D.ttf";
const FONT_CACHE = path.join(tmpdir(), "suicash-REM-wght.ttf");

const scaleArg = process.argv.find((a) => a.startsWith("--scale="));
const SCALE = scaleArg ? Number(scaleArg.split("=")[1]) : 2;
if (!Number.isFinite(SCALE) || SCALE <= 0) {
  console.error("usage: node generate-png.mjs [--scale=2]");
  process.exit(1);
}

function isFont(buf) {
  if (buf.length < 4) return false;
  const magic = buf.subarray(0, 4).toString("binary");
  return magic === "\x00\x01\x00\x00" || magic === "OTTO" || magic === "true" || magic === "wOFF";
}

async function resolveRemFont() {
  const fromEnv = process.env.SUICASH_REM_TTF;
  if (fromEnv) {
    const buf = readFileSync(fromEnv);
    if (!isFont(buf)) throw new Error(`not a font file: ${fromEnv}`);
    return fromEnv;
  }
  try {
    const cached = readFileSync(FONT_CACHE);
    if (isFont(cached)) return FONT_CACHE;
  } catch {
    // miss → download
  }
  console.log(`downloading REM variable font -> ${FONT_CACHE}`);
  const res = await fetch(REM_URL);
  if (!res.ok) throw new Error(`failed to download REM font: ${res.status}`);
  writeFileSync(FONT_CACHE, Buffer.from(await res.arrayBuffer()));
  const buf = readFileSync(FONT_CACHE);
  if (!isFont(buf)) throw new Error("downloaded file is not a font (proxy error page?)");
  return FONT_CACHE;
}

function findSvgs(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    if (entry === "node_modules") continue;
    const full = path.join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...findSvgs(full));
    else if (entry.endsWith(".svg")) out.push(full);
  }
  return out.sort();
}

const fontFile = await resolveRemFont();
const svgs = findSvgs(HERE);
if (svgs.length === 0) {
  console.error("no .svg found under assets/");
  process.exit(1);
}
mkdirSync(path.dirname(FONT_CACHE), { recursive: true });

for (const svgPath of svgs) {
  const svg = readFileSync(svgPath);
  const resvg = new Resvg(svg, {
    fitTo: { mode: "zoom", value: SCALE },
    font: { fontFiles: [fontFile] },
  });
  const png = resvg.render().asPng();
  const outPath = svgPath.replace(/\.svg$/, ".png");
  writeFileSync(outPath, png);
  const rel = path.relative(HERE, outPath);
  const px = `${png.readUInt32BE(16)}x${png.readUInt32BE(20)}`;
  console.log(`generated -> ${rel} (${px}, ${(png.length / 1024).toFixed(1)} KB)`);
}
