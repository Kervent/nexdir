// Generates every raster logo asset from the single SVG master
// (assets/branding/svg/nexdir-logo.svg). Run with: npm run branding
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";
import pngToIco from "png-to-ico";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const branding = join(root, "assets", "branding");
const pngDir = join(branding, "png");
mkdirSync(pngDir, { recursive: true });

const master = readFileSync(join(branding, "svg", "nexdir-logo.svg"), "utf8");
// Full-bleed variant: Discord crops server icons to a circle, so square corners avoid
// a visible ring of transparency. The glyph itself is unchanged.
const fullBleed = master.replace('rx="224"', 'rx="0"');

function render(svg, size, file) {
  const png = new Resvg(svg, { fitTo: { mode: "width", value: size } }).render().asPng();
  const out = join(pngDir, file);
  writeFileSync(out, png);
  console.log(`wrote ${out}`);
  return out;
}

render(master, 1024, "nexdir-logo-1024.png");
render(master, 512, "nexdir-logo-512.png");
const icoSizes = [16, 24, 32, 48, 64, 128, 256].map((s) => render(master, s, `nexdir-logo-${s}.png`));
render(fullBleed, 1024, "nexdir-discord-1024.png");
render(fullBleed, 512, "nexdir-discord-512.png");

const ico = await pngToIco(icoSizes);
writeFileSync(join(branding, "nexdir.ico"), ico);
console.log("wrote nexdir.ico");

// GitHub social preview (1280×640).
const glyph = master
  .replace(/<svg[^>]*>/, "")
  .replace("</svg>", "")
  .replace(/<!--[\s\S]*?-->/, "");
const social = `<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="640" viewBox="0 0 1280 640">
  <rect width="1280" height="640" fill="#0F1220"/>
  <g transform="translate(120 160) scale(0.3125)">${glyph}</g>
  <text x="480" y="300" fill="#F4F5FB" font-family="Segoe UI, Arial, sans-serif" font-size="112" font-weight="700">Nexdir</text>
  <text x="484" y="372" fill="#A3A9C7" font-family="Segoe UI, Arial, sans-serif" font-size="40">A smarter way to manage your files.</text>
  <text x="484" y="440" fill="#6F7699" font-family="Segoe UI, Arial, sans-serif" font-size="28">Open-source file manager for Windows</text>
</svg>`;
writeFileSync(join(branding, "svg", "nexdir-social-preview.svg"), social);
const socialPng = new Resvg(social, { font: { loadSystemFonts: true } }).render().asPng();
writeFileSync(join(pngDir, "nexdir-social-preview.png"), socialPng);
console.log("wrote social preview");

// Favicon for the web view.
mkdirSync(join(root, "public"), { recursive: true });
copyFileSync(join(branding, "svg", "nexdir-mark.svg"), join(root, "public", "nexdir.svg"));
