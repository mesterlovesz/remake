r"""Optional remastered ("HD") textures: Real-ESRGAN upscales of every retail albedo texture the game draws on 3D surfaces
(docs/retail-hd-textures.md).

    python -m tools.upscale_textures output                       # everything, level by level (resumable)
    python -m tools.upscale_textures output --worlds rh3-miasteczko0 --no-skins
    python -m tools.upscale_textures output --plan                 # only count what would be done
    python -m tools.upscale_textures output --exe D:/tools/realesrgan/realesrgan-ncnn-vulkan.exe

Writes `output/textures_hd/<same relative path as the original>` (textures/<world>/matNNNN.png, world_models/<world>/textures/...,
model_textures/skins/...), so the game swaps a texture by prefixing `textures_hd/`.  Identical textures of different worlds are
upscaled once (`textures_hd/_pool/<hash>.png`, the mirrored names are NTFS hard links, or copies where links are impossible).
Interruptible (Ctrl+C) and resumable: a finished pool file is never redone.  The GPU job runs at the lowest CPU priority.

Recipe
  * model `realesrgan-x4plus` (the model of the 2026-REMAKE texgen crate), always run at x4, RGB only;
  * source longest side <= 256: kept at x4 (<= 1024 px); <= 1024: the x4 result is Lanczos-reduced to x2 (<= 2048 px);
    anything larger (the 4096 px sky strip) is left alone.  Integer scale, same aspect ratio: UV / lightmap alignment cannot move;
  * world textures repeat: the image is wrap-padded by 16 px before the model and cropped afterwards, so opposite edges match
    (no seams when a face tiles); skins are edge-padded (they are clamped);
  * ALPHA never goes through the model: it is resized separately (bicubic, same padding), so cut-outs (fences, foliage, glass) and
    the fullbright mask keep clean edges;  for textures used as transparency (blend / mask / add) the colour of fully transparent
    texels is first bled outwards from the opaque ones, so no dark halo appears at the alpha edge;
  * each result gets a per-channel mean correction to the source colour (the model drifts by a few levels);
  * NOT touched: lightmaps, env/detail effect textures (env_textures/), sprites, HUD, menu art, cursors, fonts.
"""
import argparse, hashlib, json, os, re, shutil, subprocess, sys, tempfile, time
from pathlib import Path

import numpy as np
from PIL import Image

MODEL = 'realesrgan-x4plus'
PAD = 16                      # source pixels of wrap / edge padding around every image
RECIPE = 'x4plus-v1'          # part of the pool hash: bump to force a redo
DEFAULT_EXE = Path(os.environ.get('LOCALAPPDATA', 'C:/Users/mannin/AppData/Local')) / 'Mesterlovesz2026/tools/realesrgan/realesrgan-ncnn-vulkan.exe'
FIRST = ['rh3-miasteczko0', 'rh1-wiezienie2', 'knajpa', 'RH9-fabryka']   # the levels checked first, then skins, then the rest
BATCH = 40
IDLE_PRIORITY, NO_WINDOW = 0x40, 0x08000000


def target_scale(w, h):
    """Integer upscale factor for a source size (0 = leave it alone)."""
    m = max(w, h)
    return 4 if m <= 256 else 2 if m <= 1024 else 0


def hd_rel(rel):
    return 'textures_hd/' + rel


def is_lightmap(p):
    n = p.name.lower()
    return n.startswith('lightmap') or n.startswith('lightgroup')


def parse_mtl(text):
    out, cur = {}, None
    for line in text.splitlines():
        if line.startswith('newmtl '): cur = line[7:].strip()
        elif line.startswith('map_Kd ') and cur: out[cur] = line[7:].strip()
    return out


def roles(output):
    """texture path (relative, '/') -> 'cutout' when a material draws it with blend / mask / add alpha, else 'plain'."""
    found = {}
    for mtl in list(output.glob('*.visual.mtl')) + list(output.glob('world_models/*/*.visual.mtl')):
        js = mtl.with_name(mtl.name[:-len('.mtl')] + '.materials.json')
        try: mats = json.loads(js.read_text(encoding='utf-8'))
        except (OSError, ValueError): mats = {}
        for material, path in parse_mtl(mtl.read_text(encoding='utf-8', errors='replace')).items():
            if mats.get(material, {}).get('alpha_mode') in ('blend', 'mask', 'add'): found[path] = 'cutout'
            else: found.setdefault(path, 'plain')
    return found


def worlds(output):
    return sorted(p.name for p in (output / 'textures').iterdir() if p.is_dir())


def groups(output, wanted, skins):
    """[(group name, [(relative path, kind)])]; kind is 'world' (tiles) or 'skin' (clamped)."""
    names = wanted or worlds(output)
    order = [n for n in FIRST if n in names] + [n for n in names if n not in FIRST]
    result = []
    for w in order:
        files = list((output / 'textures' / w).glob('*.png')) + list((output / 'world_models' / w).rglob('*.png'))
        result.append((w, [(p.relative_to(output).as_posix(), 'world') for p in sorted(files) if not is_lightmap(p)]))
    if skins:
        files = sorted((output / 'model_textures' / 'skins').rglob('*.png'))
        entry = ('skins', [(p.relative_to(output).as_posix(), 'skin') for p in files])
        result.insert(min(1, len(result)), entry)        # right after the first level: its NPCs and props need them
    return result


def key_of(path, kind, role):
    h = hashlib.sha1(path.read_bytes())
    h.update(f'|{kind}|{role}|{RECIPE}|{PAD}'.encode())
    return h.hexdigest()[:20]


def bleed(rgb, alpha, threshold=8):
    """Fill the colour of (almost) transparent texels from the nearest opaque ones so filtering never pulls a dark fringe in."""
    solid = alpha > threshold
    if solid.all() or not solid.any(): return rgb
    rgb, solid = rgb.copy(), solid.copy()
    for _ in range(max(rgb.shape[:2])):
        if solid.all(): break
        acc, cnt = np.zeros(rgb.shape, np.float32), np.zeros(solid.shape, np.float32)
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                if dx == 0 and dy == 0: continue
                s = np.roll(np.roll(solid, dy, 0), dx, 1)
                c = np.roll(np.roll(rgb, dy, 0), dx, 1).astype(np.float32)
                acc += c * s[..., None]; cnt += s
        grow = ~solid & (cnt > 0)
        rgb[grow] = (acc[grow] / cnt[grow][:, None]).round().astype(np.uint8)
        solid |= grow
    return rgb


def prepare(path, kind, role):
    """-> (padded RGB array for the model, padded alpha or None, source size)"""
    im = Image.open(path).convert('RGBA')
    a = np.asarray(im)
    rgb, alpha = a[..., :3], a[..., 3]
    if role == 'cutout': rgb = bleed(rgb, alpha)
    mode = 'wrap' if kind == 'world' else 'edge'
    pr = np.pad(rgb, ((PAD, PAD), (PAD, PAD), (0, 0)), mode=mode)
    pa = None if (alpha == 255).all() else np.pad(alpha, PAD, mode=mode)
    return pr, pa, im.size


def finish(model_out, alpha_pad, src_path, size, scale):
    """Crop the padding, reduce to the target scale, correct the mean colour, attach the separately resized alpha."""
    w, h = size
    big = Image.open(model_out).convert('RGB')
    assert big.size == ((w + 2 * PAD) * 4, (h + 2 * PAD) * 4), (big.size, size)
    big = big.crop((PAD * 4, PAD * 4, (PAD + w) * 4, (PAD + h) * 4))
    if scale != 4: big = big.resize((w * scale, h * scale), Image.LANCZOS)
    src = np.asarray(Image.open(src_path).convert('RGBA'))
    weight = (src[..., 3] > 127) if (src[..., 3] > 127).any() else np.ones(src.shape[:2], bool)
    down = np.asarray(big.resize((w, h), Image.BOX)).astype(np.float32)
    delta = src[..., :3].astype(np.float32)[weight].mean(0) - down[weight].mean(0)
    out = np.clip(np.asarray(big).astype(np.float32) + np.clip(delta, -16, 16), 0, 255).round().astype(np.uint8)
    if alpha_pad is None: return Image.fromarray(out, 'RGB')
    al = Image.fromarray(alpha_pad).resize((alpha_pad.shape[1] * scale, alpha_pad.shape[0] * scale), Image.BICUBIC)
    al = np.asarray(al)[PAD * scale:(PAD + h) * scale, PAD * scale:(PAD + w) * scale]
    return Image.fromarray(np.dstack([out, al]), 'RGBA')


def run_model(exe, indir, outdir, gpu):
    cmd = [str(exe), '-i', str(indir), '-o', str(outdir), '-n', MODEL, '-s', '4', '-f', 'png', '-m', str(Path(exe).parent / 'models')]
    if gpu is not None: cmd += ['-g', str(gpu)]
    flags = (IDLE_PRIORITY | NO_WINDOW) if os.name == 'nt' else 0
    proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=flags)
    try:
        return proc.wait()
    except BaseException:
        proc.kill(); raise


def link(src, dst):
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists(): dst.unlink()
    try: os.link(src, dst)
    except OSError: shutil.copy2(src, dst)


def process(output, todo, exe, gpu, pause, log):
    """todo: {key: (path, kind, role)}.  Fills output/textures_hd/_pool.  Returns the keys that failed."""
    pool = output / 'textures_hd' / '_pool'
    pool.mkdir(parents=True, exist_ok=True)
    failed, items = [], list(todo.items())
    for i in range(0, len(items), BATCH):
        chunk = items[i:i + BATCH]
        with tempfile.TemporaryDirectory(prefix='hd_', dir=output / 'textures_hd') as tmp:
            tin, tout = Path(tmp, 'in'), Path(tmp, 'out')
            tin.mkdir(); tout.mkdir()
            prepared = {}
            for key, (path, kind, role) in chunk:
                pr, pa, size = prepare(output / path, kind, role)
                Image.fromarray(pr, 'RGB').save(tin / f'{key}.png', compress_level=1)
                prepared[key] = (pa, size)
            code = run_model(exe, tin, tout, gpu)
            for key, (path, kind, role) in chunk:
                produced = tout / f'{key}.png'
                if not produced.is_file():
                    log(f'  FAILED {path} (model exit {code})'); failed.append(key); continue
                pa, size = prepared[key]
                scale = target_scale(*size)
                image = finish(produced, pa, output / path, size, scale)
                tmp_file = pool / f'{key}.png.part'
                image.save(tmp_file, format='PNG', compress_level=6)
                os.replace(tmp_file, pool / f'{key}.png')
        time.sleep(pause)
    return failed


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('output', nargs='?', default='output')
    ap.add_argument('--exe', default=str(DEFAULT_EXE), help='realesrgan-ncnn-vulkan executable (models/ next to it)')
    ap.add_argument('--worlds', nargs='*', help='only these worlds (default: all, first levels first)')
    ap.add_argument('--no-skins', action='store_true', help='skip model / prop / character skins')
    ap.add_argument('--gpu', type=int, default=None, help='Vulkan device index (default: the tool picks, usually the discrete GPU)')
    ap.add_argument('--pause', type=float, default=1.0, help='seconds of rest between batches (keeps the desktop responsive)')
    ap.add_argument('--plan', action='store_true', help='only count the work')
    a = ap.parse_args()
    output = Path(a.output).resolve()
    hd = output / 'textures_hd'
    hd.mkdir(exist_ok=True)
    logfile = hd / 'upscale.log'

    def log(msg):
        line = f'{time.strftime("%H:%M:%S")} {msg}'
        print(line, flush=True)
        with open(logfile, 'a', encoding='utf-8') as f: f.write(line + '\n')

    exe = Path(a.exe)
    if not a.plan and not exe.is_file(): sys.exit(f'Real-ESRGAN binary not found: {exe} (download realesrgan-ncnn-vulkan, use --exe)')
    role_of = roles(output)
    manifest_path = hd / 'manifest.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8')) if manifest_path.is_file() else {'recipe': RECIPE, 'model': MODEL, 'files': {}, 'timings': {}}
    started = time.time()
    try:
        for name, files in groups(output, a.worlds, not a.no_skins):
            t0 = time.time()
            entries, todo, skipped = [], {}, 0
            for rel, kind in files:
                src = output / rel
                w, h = Image.open(src).size
                if target_scale(w, h) == 0: skipped += 1; continue
                role = role_of.get(rel, 'plain')
                key = key_of(src, kind, role)
                entries.append((rel, key, w, h))
                if not (hd / '_pool' / f'{key}.png').is_file(): todo[key] = (rel, kind, role)
            log(f'== {name}: {len(files)} textures ({len(entries)} eligible, {skipped} too large), {len(todo)} to upscale')
            if a.plan: continue
            failed = process(output, todo, exe, a.gpu, a.pause, log) if todo else []
            size = 0
            for rel, key, w, h in entries:
                pooled = hd / '_pool' / f'{key}.png'
                if key in failed or not pooled.is_file(): continue
                link(pooled, output / hd_rel(rel))
                size += pooled.stat().st_size
                manifest['files'][rel] = {'key': key, 'src': [w, h], 'scale': target_scale(w, h)}
            took = time.time() - t0
            manifest['timings'][name] = {'seconds': round(took, 1), 'upscaled': len(todo) - len(failed), 'linked': len(entries) - len(failed), 'bytes': size}
            manifest_path.write_text(json.dumps(manifest, indent=1), encoding='utf-8')
            log(f'== {name} done in {took:.0f} s: {len(todo) - len(failed)} upscaled, {len(entries)} in place, {size / 1e6:.0f} MB, {len(failed)} failed')
    except KeyboardInterrupt:
        log('interrupted: finished textures are kept, run again to continue')
        sys.exit(130)
    log(f'all done in {(time.time() - started) / 60:.1f} min')


if __name__ == '__main__':
    main()
