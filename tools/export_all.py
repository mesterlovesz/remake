"""One command for the whole first-run setup of "A Mesterlövész: Újratöltve" (called by Indit.cmd).

    python -m tools.export_all [--game ../GYARI] [--output output] [--run] [--force] [--skip-build] [--only LÉPÉS] [--list] [--jobs N]

What it does, in order (each step is skipped when it is already finished, so a stopped run simply continues):
  1. checks: Python, helper packages (pip), the original game folder, Rust, free disk space (tools/setup_check.py)
  2. exports every needed file from the ORIGINAL game into `output/` (the exporters of tools/, steps `scripts` .. `audio`)
  3. builds the game: `cargo build --release` (crates/level-viewer/target/release/level-viewer.exe)
  4. with --run: starts the game (main menu)

The original game folder is only read. Finished steps leave `output/.setup/<step>.done`; a step is redone when its marker is
missing, when the original game files changed, or with --force. Logs: `output/.setup/logs/`.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from . import setup_check

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / 'crates' / 'level-viewer'
# Bump when an exporter changes in a way old exports must be redone.
EXPORT_VERSION = 1
# The 27 campaign worlds plus Chinatown's first world (tools/export_campaign.py): the ones with gameplay data.
CAMPAIGN = ('rh3-miasteczko0', 'rh1-wiezienie1', 'rh1-wiezienie2', 'rh1-wiezienie3', 'rh2-wiezienie1', 'rh2-wiezienie2', 'rh3-miasteczko1',
            'rh3-miasteczko2', 'burmistrz1', 'burmistrz2', 'chapel_mniejszy', 'knajpa', 'Rh7a-Tunele', 'podziemia1', 'podziemia1a', 'podziemia1b',
            'podziemia1c', 'chinatown2', 'RH9-fabryka', 'rh10-wiezowiec1', 'rh10-wiezowiec2', 'rh10-wiezowiec3', 'wiez_wn1', 'wiez_wn2', 'wiez_wn3',
            'rh12-lab1', 'rh12-lab2', 'chinatown')


class StepFailed(Exception):
    pass


def human(seconds: float) -> str:
    seconds = int(round(seconds))
    return f'{seconds // 60} p {seconds % 60:02d} mp' if seconds >= 60 else f'{seconds} mp'


class Context:
    def __init__(self, game: Path, output: Path, jobs: int, force: bool):
        self.game, self.output, self.jobs, self.force = game, output.resolve(), jobs, force
        self.state = self.output / '.setup'
        self.logs = self.state / 'logs'
        self.logs.mkdir(parents=True, exist_ok=True)
        self.env = child_env()

    def fingerprint(self) -> str:
        """Identifies the original install: sizes of its three code files plus the export version."""
        sizes = [(self.game / name).stat().st_size for name in ('Lithtech.exe', 'cshell.dll', 'object.lto')]
        return hashlib.sha1(json.dumps([EXPORT_VERSION, sizes]).encode()).hexdigest()[:16]

    def marker(self, name: str) -> Path:
        return self.state / f'{name}.done'

    def is_done(self, name: str) -> bool:
        path = self.marker(name)
        if self.force or not path.is_file():
            return False
        try:
            return json.loads(path.read_text(encoding='utf-8')).get('fingerprint') == self.fingerprint()
        except (OSError, ValueError):
            return False

    def mark(self, name: str, seconds: float):
        self.marker(name).write_text(json.dumps({'fingerprint': self.fingerprint(), 'seconds': round(seconds, 1), 'finished': time.strftime('%Y-%m-%d %H:%M:%S')}), encoding='utf-8')

    def run(self, log: str, *args, cwd=ROOT, tolerate=None) -> bool:
        """`python -m <args>` with its output in logs/<log>.log; StepFailed with the log tail on a non-zero exit."""
        command = [sys.executable, '-X', 'utf8', '-m', *map(str, args)]
        path = self.logs / f'{log}.log'
        with path.open('w', encoding='utf-8') as stream:
            stream.write('$ ' + ' '.join(command) + '\n')
            stream.flush()
            code = subprocess.run(command, cwd=cwd, stdout=stream, stderr=subprocess.STDOUT, env=self.env).returncode
        if code != 0:
            tail = '\n'.join(path.read_text(encoding='utf-8', errors='replace').splitlines()[-14:])
            if tolerate and tolerate in tail:
                return False
            hint = ''
            if 'No space left' in tail or 'out of disk' in tail.lower():
                hint = '\n  Elfogyott a szabad hely: szabadíts fel helyet, majd indítsd újra az Indit.cmd-t (a kész lépéseket kihagyja).'
            raise StepFailed(f'a(z) {args[0]} hibával állt le (kilépési kód {code}). Napló: {path}\n{tail}{hint}')
        return True


def child_env():
    env = dict(os.environ)
    env['PYTHONUTF8'] = '1'
    env['PYTHONPATH'] = str(ROOT) + os.pathsep + env.get('PYTHONPATH', '')
    return env


# ---- the steps -------------------------------------------------------------------------------------------------------------------

def world_files(ctx: Context):
    return sorted((ctx.game / 'worlds').glob('*.dat'), key=lambda path: path.name.lower())


def export_world(ctx: Context, dat: Path):
    out, game, name = ctx.output, ctx.game, dat.stem
    ctx.run(f'world-{name}-world', 'tools.export_world', dat, out)
    # katscena.dat is an older DAT version the render reader does not support: only its scene data is exported (as in the owner's export).
    if ctx.run(f'world-{name}-visual', 'tools.export_visual', dat, game, out, tolerate='render reader supports DAT'):
        ctx.run(f'world-{name}-model_worlds', 'tools.export_model_worlds', dat, game, out)
        ctx.run(f'world-{name}-light_groups', 'tools.export_light_groups', dat, out)
        ctx.run(f'world-{name}-props', 'tools.export_props', dat, game, out)


def step_scripts(ctx: Context):
    ctx.run('scripts', 'tools.decode_scripts', ctx.game, ctx.output / 'decoded_scripts')


def step_worlds(ctx: Context):
    worlds = world_files(ctx)
    todo = [dat for dat in worlds if ctx.force or not ctx.marker(f'world-{dat.stem}').is_file()]
    done, total, started = len(worlds) - len(todo), len(worlds), time.time()

    def one(dat):
        export_world(ctx, dat)
        ctx.marker(f'world-{dat.stem}').write_text(ctx.fingerprint(), encoding='utf-8')
        return dat.stem

    with ThreadPoolExecutor(max_workers=ctx.jobs) as pool:
        futures = [pool.submit(one, dat) for dat in todo]
        for future in futures:
            name = future.result()
            done += 1
            print(f'      {done}/{total} {name} kész ({human(time.time() - started)} telt el)', flush=True)


def step_campaign(ctx: Context):
    ctx.run('campaign', 'tools.export_campaign', ctx.game, ctx.output, '--worlds', *CAMPAIGN)


def simple(module, *, flags=False, name=None):
    def run(ctx: Context):
        if flags:
            ctx.run(name or module, f'tools.{module}', '--game', ctx.game, '--output', ctx.output)
        else:
            ctx.run(name or module, f'tools.{module}', ctx.game, ctx.output)
    return run


def step_props_defs(ctx: Context):
    ctx.run('props_defs', 'tools.export_props', 'definitions', ctx.game, ctx.output)


def step_build(ctx: Context):
    exe = viewer_exe()
    env = cargo_env()
    jobs = os.environ.get('MESTER_JOBS')
    command = [shutil.which('cargo', path=env.get('PATH')) or 'cargo', 'build', '--release'] + (['-j', jobs] if jobs else [])
    print('      cargo build --release (a kimenetét lent látod)', flush=True)
    code = subprocess.run(command, cwd=CRATE, env=env).returncode
    if code != 0 or not exe.is_file():
        raise StepFailed('a fordítás nem sikerült (a fenti cargo hibaüzenet mutatja az okát).\n'
                         '  Gyakori okok: hiányzik a Visual Studio C++ Build Tools (a rustup telepítője felajánlja), elavult Rust (rustup update), megszakadt internet, elfogyott a hely.\n'
                         '  Az Indit.cmd újraindítása folytatja a fordítást.')


def source_digest() -> str:
    """Hash of the Rust sources + lockfiles: the build is redone only when the code changed (git pull)."""
    digest = hashlib.sha1()
    for base in sorted(ROOT.glob('crates/*')):
        for path in sorted([*base.glob('Cargo.*'), *base.rglob('*.rs')]):
            if 'target' in path.relative_to(base).parts:
                continue
            digest.update(str(path.relative_to(ROOT)).encode())
            digest.update(path.read_bytes())
    return digest.hexdigest()[:16]


def viewer_exe() -> Path:
    target = Path(os.environ.get('CARGO_TARGET_DIR') or CRATE / 'target')
    return target / 'release' / 'level-viewer.exe'


def cargo_env():
    """The environment cargo runs in: the owner's project-local toolchain when that folder exists, else the normal rustup install."""
    env = child_env()
    local = ROOT.parent.parent / '_toolchain'
    if (local / 'cargo' / 'bin' / 'cargo.exe').is_file():
        env['CARGO_HOME'], env['RUSTUP_HOME'] = str(local / 'cargo'), str(local / 'rustup')
        env['PATH'] = str(local / 'cargo' / 'bin') + os.pathsep + env['PATH']
    else:
        home = Path(os.environ.get('CARGO_HOME') or Path.home() / '.cargo') / 'bin'
        if home.is_dir():
            env['PATH'] = str(home) + os.pathsep + env['PATH']
    return env


EXPORT_STEPS = [
    # (id, title, estimate, run)
    ('scripts', 'A gyári szkriptek visszafejtése', 'néhány mp', step_scripts),
    ('worlds', 'A pályák kinyerése (geometria, textúrák, fénytérképek, tárgyak)', 'kb. 1-4 perc', step_worlds),
    ('presentation', 'HUD és nyitójelenet', 'néhány mp', simple('export_presentation')),
    ('campaign', 'Szereplők, küldetések, párbeszédek, modellek', 'kb. 1-3 perc', step_campaign),
    ('scenes', 'Átvezetők és a befejezés', 'néhány mp', simple('export_scenes')),
    ('weapons', 'Fegyverek', 'néhány mp', simple('export_weapons', flags=True)),
    ('effects', 'Lövés-, vér- és füsteffektek', 'néhány mp', simple('export_effects')),
    ('altfire', 'Lézer, távcső, gumibot', 'néhány mp', simple('export_altfire')),
    ('decorations', 'Díszletek (lámpafény, eső, lens flare)', 'néhány mp', simple('export_decorations')),
    ('env_effects', 'Textúraeffektek (környezettükör, részlet)', 'néhány mp', simple('export_env_effects', flags=True)),
    ('inventory', 'Felszerelés- és karakterképernyők', 'néhány mp', simple('export_inventory')),
    ('menu', 'Főmenü és betöltőképek', 'néhány mp', simple('export_menu')),
    ('cursor', 'Egérkurzor', 'néhány mp', simple('export_cursor')),
    ('props_defs', 'Tárgydefiníciók', 'néhány mp', step_props_defs),
    ('videos', 'Indító logóvideók (ffmpeg)', 'néhány mp', simple('export_videos')),
    ('audio', 'Hangok és zene (kb. 1100 fájl másolása)', 'néhány mp', simple('export_audio', flags=True)),
]
# Files every finished step must have produced (checked right after the step, so a silently failed exporter is noticed at once).
EXPECT = {
    'scripts': ['decoded_scripts/items.txt', 'decoded_scripts/objects.txt', 'decoded_scripts/text_keys.txt'],
    'campaign': ['actors.json', 'items.json', 'campaign_cutscenes.json', 'rh3-miasteczko0.gameplay.json', 'rh1-wiezienie1.items.json', 'models'],
    'presentation': ['opening.json', 'hud/pasek_health.png'],
    'scenes': ['endgame.json', 'ui/outro'],
    'weapons': ['retail_weapons.json', 'retail_items.json'],
    'effects': ['retail_effects.json'],
    'altfire': ['retail_altfire.json'],
    'decorations': ['decor/sprites.json'],
    'env_effects': ['env_effects.json'],
    'inventory': ['retail_inventory.json'],
    'menu': ['retail_ui.json', 'ui/menu/main_menu_1024.png'],
    'cursor': ['ui/cursor.png'],
    'props_defs': ['props_defs.json'],
    'videos': ['videos/videos.json'],
    'audio': ['audio/sounds'],
}
# A world must at least have its scene data; every world whose DAT version the render reader supports also its geometry.
WORLD_FILES = ['scene.json', 'collision.obj', 'visual.obj', 'visual.materials.json', 'render_models.json']


def verify(ctx: 'Context', name: str):
    missing = [path for path in EXPECT.get(name, []) if not (ctx.output / path).exists()]
    if name == 'worlds':
        for dat in world_files(ctx):
            files = WORLD_FILES[:1] if dat.stem == 'katscena' else WORLD_FILES
            missing += [f'{dat.stem}.{suffix}' for suffix in files if not (ctx.output / f'{dat.stem}.{suffix}').exists()]
    if missing:
        raise StepFailed('a lépés lefutott, de ezek a fájlok nem készültek el: ' + ', '.join(missing[:8]) + '. Töröld az output\\.setup mappát, és indítsd újra az Indit.cmd-t; ha így is hibázik, nézd meg a naplót: output\\.setup\\logs.')


BUILD = ('build', 'A játék lefordítása (cargo build --release)', 'kb. 10-30 perc, csak egyszer')


def start_game(output: Path) -> int:
    exe = viewer_exe()
    env = dict(os.environ, WGPU_BACKEND=os.environ.get('WGPU_BACKEND', 'vulkan'))
    print(f'A játék indul: {exe.name} menu', flush=True)
    return subprocess.call([str(exe), 'menu', str(output)], cwd=CRATE, env=env)


def ready_file(output: Path) -> Path:
    return Path(output) / '.setup' / 'ready.json'


def is_ready(output: Path) -> bool:
    """Everything done earlier for exactly this code and export version: the next start only launches the game."""
    try:
        info = json.loads(ready_file(output).read_text(encoding='utf-8'))
    except (OSError, ValueError):
        return False
    return info.get('export_version') == EXPORT_VERSION and info.get('source_digest') == source_digest() and viewer_exe().is_file()


# Files whose presence means "a complete export of this project already lives here" (e.g. the developer's own output/): adopted, not redone.
ADOPT = ['retail_weapons.json', 'retail_ui.json', 'retail_inventory.json', 'endgame.json', 'actors.json', 'props_defs.json', 'videos/videos.json',
         'decoded_scripts/gameai.txt', 'rh1-wiezienie1.scene.json', 'rh3-miasteczko0.visual.obj', 'audio/sounds']


def adopt_existing(ctx: Context) -> bool:
    if ctx.force or any(ctx.state.glob('*.done')) or not all((ctx.output / path).exists() for path in ADOPT):
        return False
    for dat in world_files(ctx):
        ctx.marker(f'world-{dat.stem}').write_text(ctx.fingerprint(), encoding='utf-8')
    for name, *_ in EXPORT_STEPS:
        ctx.mark(name, 0)
    return True


def game_exit_message(code: int):
    if code != 0:
        print(f'A játék hibakóddal állt le ({code}). Ha a videokártya/Vulkan a gond: frissítsd a videokártya-illesztőprogramot (lásd README, Hibaelhárítás).')
    return code


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding='utf-8', errors='replace')
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--game', type=Path, help='az eredeti játék mappája (alapból ../GYARI)')
    parser.add_argument('--output', type=Path, default=ROOT / 'output')
    parser.add_argument('--run', action='store_true', help='kész után elindítja a játékot')
    parser.add_argument('--force', action='store_true', help='minden lépést újrafuttat')
    parser.add_argument('--skip-build', action='store_true', help='nem fordít (csak az export)')
    parser.add_argument('--only', help='csak ezt a lépést futtatja (lásd --list)')
    parser.add_argument('--list', action='store_true')
    parser.add_argument('--jobs', type=int, default=min(3, max(1, (os.cpu_count() or 2) // 2)), help='párhuzamos pályaexportok')
    args = parser.parse_args()
    if args.list:
        for name, title, estimate, _ in [*EXPORT_STEPS, (*BUILD, None)]:
            print(f'{name:14} {title} ({estimate})')
        return 0
    output = args.output.resolve()

    if not (args.force or args.only or args.skip_build) and is_ready(output):  # every later start: no checks, no export, no build
        if not args.run:
            print('Minden kész: a játék lefordítva, az adatok kinyerve. Indítás: Indit.cmd')
            return 0
        return game_exit_message(start_game(output))

    print('=== A Mesterlövész: Újratöltve - előkészítés ===', flush=True)
    print('Az eredeti játékfájlokat csak olvassa, nem módosítja őket. Ami már kész, azt kihagyja.\n', flush=True)
    stopped = time.time()
    steps = [step for step in EXPORT_STEPS if not args.only or step[0] == args.only]
    total = 1 + len(steps) + (0 if args.skip_build or args.only else 1)
    # 1. checks
    print(f'[1/{total}] Ellenőrzés (Python, csomagok, az eredeti játék, Rust, szabad hely)', flush=True)
    problems = setup_check.check_python()
    game, game_problems = setup_check.check_game(args.game)
    problems += game_problems
    ctx = None
    if not problems:
        todo = setup_check.missing_packages()
        if todo:
            print('      Hiányzó Python csomagok telepítése (pip, internet kell):', flush=True)
            problems += setup_check.install_packages(todo)
    build_pending = False
    if not problems:
        ctx = Context(game, output, max(1, args.jobs), args.force)
        if adopt_existing(ctx):
            print('      Már van kész export ebben a mappában, ezt használom (újrakinyerés: Indit.cmd --force).', flush=True)
        export_pending = any(not ctx.is_done(name) for name, *_ in steps)
        build_digest = source_digest()
        build_marker = ctx.state / 'build.done'
        build_pending = not (args.skip_build or args.only) and (args.force or not viewer_exe().is_file() or not build_marker.is_file()
                                                                or build_marker.read_text(encoding='utf-8').strip() != build_digest)
        if build_pending:
            problems += setup_check.check_rust(cargo_env())
        problems += setup_check.check_disk(output if export_pending else None, viewer_exe().parent if build_pending else None)
    if problems:
        print()
        for problem in problems:
            print('HIBA: ' + problem + '\n')
        print('Javítsd a fentieket, majd indítsd újra az Indit.cmd-t.')
        return 2
    print(f'      Rendben. Eredeti játék: {game}  ({setup_check.game_version(game)})', flush=True)
    for warning in setup_check.check_version(game):
        print('      FIGYELEM: ' + warning, flush=True)

    for index, (name, title, estimate, run) in enumerate(steps, 2):
        label = f'[{index}/{total}] {title}'
        if ctx.is_done(name) and not (args.only and ctx.force):
            print(f'{label}: már kész, kihagyom', flush=True)
            continue
        print(f'{label} ({estimate})', flush=True)
        began = time.time()
        try:
            run(ctx)
            verify(ctx, name)
        except StepFailed as error:
            print(f'\nHIBA a(z) „{title}” lépésben: {error}\n\nA kész lépések megmaradnak: az Indit.cmd újraindítása onnan folytatja, ahol megállt.')
            return 3
        except KeyboardInterrupt:
            print('\nMegszakítva. Az Indit.cmd újraindítása onnan folytatja, ahol megállt.')
            return 130
        ctx.mark(name, time.time() - began)
        print(f'      kész ({human(time.time() - began)})', flush=True)

    if not args.skip_build and not args.only:
        name, title, estimate = BUILD
        label = f'[{total}/{total}] {title}'
        if not build_pending:
            print(f'{label}: már kész, kihagyom', flush=True)
        else:
            print(f'{label} ({estimate})', flush=True)
            began = time.time()
            try:
                step_build(ctx)
            except StepFailed as error:
                print(f'\nHIBA a fordításnál: {error}')
                return 4
            except KeyboardInterrupt:
                print('\nMegszakítva. Az Indit.cmd újraindítása onnan folytatja.')
                return 130
            build_marker.write_text(build_digest, encoding='utf-8')
            print(f'      kész ({human(time.time() - began)})', flush=True)
        if all(ctx.is_done(step[0]) for step in EXPORT_STEPS):
            ready_file(output).write_text(json.dumps({'export_version': EXPORT_VERSION, 'source_digest': build_digest, 'finished': time.strftime('%Y-%m-%d %H:%M:%S')}), encoding='utf-8')
    print(f'\nMinden kész ({human(time.time() - stopped)}).', flush=True)
    if args.run:
        return game_exit_message(start_game(output))
    return 0


if __name__ == '__main__':
    sys.exit(main())
