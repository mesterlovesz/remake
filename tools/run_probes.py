"""Runs every headless probe scenario of the level-viewer and prints PASS/FAIL per scenario.

Each scenario is one hidden, silent capture run (`level-viewer <world|menu> <output> <png> <times>` with
MESTER_TEST_SCENARIO and MESTER_SILENT=1, under a hard timeout).  The exit code of the game is the verdict
(the capture code exits with an error when the probe reports a failure or never finished), AND the log must
contain the probe's own completion line - a scenario name the game does not know would otherwise "pass" silently.
The game clock of a scenario run advances a fixed 0.05 s per frame, so a run is reproducible; only its wall-clock
duration depends on the machine.  Use the optimized dev build (--build), the plain debug build is ~7x slower.

  python -m tools.run_probes --build           # build the fast exe first (uses $CARGO_TARGET_DIR), then run all
  python -m tools.run_probes                   # all scenarios
  python -m tools.run_probes retail bus        # only these (labels of the table)
  python -m tools.run_probes --list            # table of scenarios / worlds / command lines
  python -m tools.run_probes --exe PATH        # another level-viewer.exe (default: $CARGO_TARGET_DIR/debug/level-viewer.exe)
  python -m tools.run_probes --logs DIR        # where logs and screenshots go (default: <tmp>/mester-probe-logs)

After merging another branch, add its new scenario to SCENARIOS below (the script lists source scenario names
that are not registered yet, see "unregistered").  Never opens a window and never plays sound.
"""
import argparse, os, re, shutil, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ANSI = re.compile(r"\x1b\[[0-9;]*m")
FAST = ["--config", 'profile.dev.package."*".opt-level=2', "--config", "profile.dev.opt-level=1"]

# label, scenario (MESTER_TEST_SCENARIO), world (or "menu"), extra environment, regex the log must contain, note
SCENARIOS = [
    ("campaign", "campaign", "rh1-wiezienie2", {}, r"KAMPÁNYPRÓBA: world=", "cell -> guard -> Glock kill -> three level exits by real door names"),
    ("cell", "cell", "rh1-wiezienie2", {}, r"CELLAPRÓBA:", "walk out of the cell from the real StartPoint, no cheats"),
    ("bus", "bus", "rh2-wiezienie2", {}, r"BUSZPRÓBA:", "bus marker cutscene -> miasteczko1 -> miasteczko2 -> burmistrz1 (must start on rh2-wiezienie2)"),
    ("catalog-a", "catalog", "rh3-miasteczko0", {"MESTER_TEST_FROM": "0", "MESTER_TEST_TO": "10"}, r"CATALOG FINISHED", "levels 0..9 of the list: mission script binds, actors load"),
    ("catalog-b", "catalog", "rh3-miasteczko0", {"MESTER_TEST_FROM": "10", "MESTER_TEST_TO": "20"}, r"CATALOG FINISHED", "levels 10..19 of the list"),
    ("catalog-c", "catalog", "rh3-miasteczko0", {"MESTER_TEST_FROM": "20", "MESTER_TEST_TO": "28"}, r"CATALOG FINISHED", "levels 20..27 of the list"),
    ("uv", "uv", "rh3-miasteczko0", {}, r"UV PROBE finished", "two fixed vantage points (texture coordinate check for screenshots)"),
    ("npc", "npc", "rh1-wiezienie2", {"MESTER_TEST_NPC": "o_postac22"}, r"NPC PROBE LATE", "places the player in front of a named NPC and reports its phase"),
    ("impact", "impact", "rh1-wiezienie3", {}, r"IMPACT PROBE shots=", "Glock shot at the nearest wall next to o_postac18 must fire and hit"),
    ("gunfire", "gunfire", "rh1-wiezienie2", {}, r"GUNFIRE PROBE shots=", "Glock against o_postac22 for 9 s (shots and hits must register)"),
    ("zmienna", "zmienna", "rh3-miasteczko0", {}, r"ZMIENNA PROBE markers=", "stands in every o_marker_zmienna and checks the mission variables"),
    ("lever", "lever", "wiez_wn1", {"MESTER_TEST_LEVER": "o_obiekt1"}, r"LEVER PROBE o_obiekt1:", "lever pulled with E, something must open or toggle"),
    ("stella", "stella", "chapel_mniejszy", {}, r"STELLA PROBE zagadana=", "chapel dialogue with Stella until LaskaChapelZagadana"),
    ("retail", "retail", "rh1-wiezienie2", {}, r"GYÁRI PRÓBA kész", "pickups by touch, NPC drop, Glock/FN/SIG ammo and reloads, pause, level changes keep weapons, grenade"),
    ("inventory", "inventory", "rh1-wiezienie3", {}, r"PANELPRÓBA 13:", "C / X / Z screens, weight, level-up steps, thrown item picked up again"),
    ("menu", "menu", "menu", {}, r"FŐMENÜPRÓBA: world=", "main menu -> new player -> Előszó, Kiskína from the level list, walk from its start"),
    ("input", "input", "rh1-wiezienie2", {}, r"BEMENETPRÓBA kész: hiba=false", "input_probe: Space jumps, right mouse scopes, X frees the cursor, inventory refuses the jump, Escape order", "12"),
    ("hud", "hud", "rh1-wiezienie2", {}, r"HUDPRÓBA t=10\.5", "hud fade script (logs the alpha of every HUD group; no assertion)", "11"),
    ("menu_pages", "menu_pages", "menu", {"SCRATCH": "1"}, r"MENÜPRÓBA menu_pages: kész, hiba=None", "scripted pointer walks every options page, files written (scratch MESTER_USER_DIR)"),
    ("bonus", "bonus", "menu", {"SCRATCH": "1"}, r"MENÜPRÓBA bonus: kész, hiba=None", "main menu -> Bónusz page -> Kiskína (chinatown, 17 NPCs, StartPoint) -> F5/death/F9 use save/bonus.sav -> b_door0 returns to the main menu (owner-requested bonus menu)", "3.3,7.8,9.3,15"),
    ("menu_hover", "menu_hover", "menu", {"SCRATCH": "1", "MESTER_WINDOW": "1920x1080", "MESTER_PROBE_REAL_POINTER": "1"}, r"MENÜPRÓBA menu_hover: kész, hiba=None", "pointer sweeps the main list frame by frame: no frame may show a row without its glyphs (owner: rows vanish on mouse move)"),
    ("menu_saves", "menu_saves", "rh1-wiezienie2", {"SCRATCH": "1"}, r"MENÜPRÓBA menu_saves: kész, hiba=None", "F5 / F9 quick save, menu save and load (scratch MESTER_USER_DIR)"),
    ("menu_death", "menu_death", "rh1-wiezienie2", {"SCRATCH": "1"}, r"MENÜPRÓBA menu_death: kész, hiba=None", "death message and F9 reload (scratch MESTER_USER_DIR)"),
    ("props", "props", "rh1-wiezienie2", {"MESTER_TEST_PROP": "o_obiekt0"}, r"PROPS PROBE walk_distance", "shoots a prop and logs its hit points (capture aid)"),
    ("door", "door", "rh1-wiezienie2", {"MESTER_TEST_DOOR": "b_door17", "MESTER_TEST_RETREAT": "1"}, r"DOOR PROBE t=", "walks up to a door and presses E (capture aid)"),
    ("pickup", "pickup", "rh1-wiezienie2", {"MESTER_TEST_ITEM": "medpack"}, r"PICKUP PROBE target", "stands on an item, it is taken by touch (capture aid)"),
    ("arsenal", "arsenal", "rh1-wiezienie2", {}, r"ARSENAL PRÓBA", "every weapon acquired, lowered/raised and fired (capture aid)", "62"),
    ("noise", "noise", "rh1-wiezienie2", {}, r"NOISE PRÓBA kész", "a shot behind a guard wakes him (AI noise); waits up to 8 s for his reaction", "9"),
    ("revolver", "revolver", "rh1-wiezienie2", {}, r"REVOLVER PRÓBA vége", "S&W fired once and reloaded, six casings"),
    ("move", None, "rh1-wiezienie2", {"MESTER_MOVE_PROBE": "1"}, r"MOVE PROBE t=", "scripted walk, run, jump, crouch, stand (logs the controller state)", "16"),
    ("dialogue", "dialogue", "rh1-wiezienie3", {"MESTER_TEST_DIALOG": "Wiezien27", "MESTER_TEST_PERSON": "wiezien_kuchnia"}, r"DIALOGUE PROBE started", "starts a dialogue node in front of a character and logs the panel state (capture aid)", "6"),
    ("ai-fight", "ai", "rh1-wiezienie3", {"MESTER_TEST_AI": "fight"}, r"AI PROBE RESULT fight", "an armed character in contact range: contact, reaction phase, shot rate 1/strzal (docs/retail-ai.md)", "3"),
    ("ai-far", "ai", "rh3-miasteczko2", {"MESTER_TEST_AI": "far"}, r"AI PROBE RESULT far", "the same beyond 1.3 * contact distance: no contact may happen", "3"),
    ("ai-patrol", "ai", "rh1-wiezienie3", {"MESTER_TEST_AI": "patrol"}, r"AI PROBE RESULT patrol", "a patrolling character walks its path graph at the phase speed", "3"),
    ("walk", "walk", "rh1-wiezienie2", {"MESTER_WALK_COUNT": "1"}, r"WALK SUMMARY", "walk-through planner plays one campaign level with the real controller (MESTER_WALK_COUNT levels from the given world; see docs/retail-scenes.md)", "3"),
    ("walk-chinatown", "walk", "chinatown", {"MESTER_WALK_LEVELS": "chinatown", "MESTER_WALK_SPEED": "3", "MESTER_WALK_BUDGET": "300", "MESTER_WALK_TIMEOUT": "400"}, r"WALK SUMMARY", "the cut level Kiskína (chinatown.dat, not on the campaign route): walk-through planner from its StartPoint to b_door0 -> chinatown2 (docs/cut-content.md)", "3"),
    ("altfire", "altfire", "rh1-wiezienie2", {}, r"ALTFIRE PRÓBA altfire:", "M-14 scope, laser, reload quirk (its script runs to 11 s: the probe ends at capture time + 2 s)", "10"),
    ("melee", "melee", "rh1-wiezienie2", {}, r"ALTFIRE PRÓBA melee:", "nightstick against an actor and a wall"),
    ("flash", "flash", "rh1-wiezienie2", {}, r"ALTFIRE PRÓBA flash:", "flashlight toggled with the middle button"),
    ("sounds", "sounds", "rh1-wiezienie2", {}, r"SOUNDS PRÓBA kész: hiba=false", "sound parity (docs/retail-audio-parity.md): jump, landing, footsteps, heartbeat, the three wounds, music switch, every gun's shot and reload, baton, grenade blast", "3"),
    ("start-podziemia1b", "startpose", "podziemia1b", {}, r"STARTPOSE PROBE world=podziemia1b ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-rh1-wiezienie1", "startpose", "rh1-wiezienie1", {}, r"STARTPOSE PROBE world=rh1-wiezienie1 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-rh1-wiezienie2", "startpose", "rh1-wiezienie2", {}, r"STARTPOSE PROBE world=rh1-wiezienie2 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-rh10-wiezowiec1", "startpose", "rh10-wiezowiec1", {}, r"STARTPOSE PROBE world=rh10-wiezowiec1 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-rh2-wiezienie2", "startpose", "rh2-wiezienie2", {}, r"STARTPOSE PROBE world=rh2-wiezienie2 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-rh3-miasteczko0", "startpose", "rh3-miasteczko0", {}, r"STARTPOSE PROBE world=rh3-miasteczko0 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-rh3-miasteczko1", "startpose", "rh3-miasteczko1", {}, r"STARTPOSE PROBE world=rh3-miasteczko1 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-wiez_wn1", "startpose", "wiez_wn1", {}, r"STARTPOSE PROBE world=wiez_wn1 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-wiez_wn2", "startpose", "wiez_wn2", {}, r"STARTPOSE PROBE world=wiez_wn2 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
    ("start-wiez_wn3", "startpose", "wiez_wn3", {}, r"STARTPOSE PROBE world=wiez_wn3 ", "start on the StartPoint facing its Kierunek (docs/retail-start-view.md)", "1"),
]
# Scenarios that fail on the merged tree for a game reason that is not a stale probe (details in docs/probes.md).
KNOWN_OPEN = {
    "campaign": "Wiezien27 never plays: `ifplayerseenby wiezien_kuchnia` needs the AI contact flag, which npcs/ai.rs sets only for actors with on_kontakt / a contact cone",
    "noise": "a single player shot (one-frame stimulus) no longer provokes an idle guard after the AI merge (contact needs two ticks)",
}
# Sound cues (`CUE ...` lines of MESTER_CUE_LOG=1, see audio::note_cue) a scenario's log must contain on top of its own verdict: the
# retail sound parity net. Each entry is a regex; "3d r=N" cues carry the retail radius, "2d" cues are local sounds.
CUES = {
    "sounds": [r"CUE music play wiezienie1_spokoj", r"CUE 2d \S*hero/skok\.wav", r"CUE 2d \S*hero/spad\.wav", r"CUE 2d \S*mason/SERDUCHO\.WAV", r"CUE 2d \S*hero/KROK",
               r"CUE 3d r=640 d=\d+ \S*hero/wcialo\.wav", r"CUE 2d \S*weapons/wcialo\.wav", r"CUE music stop",
               r"CUE 3d r=640 d=\d+ \S*glock_s\.wav", r"CUE 2d \S*glock_m\.wav", r"CUE 3d r=640 d=\d+ \S*s&w_s1\.wav", r"CUE 2d \S*s&w_m\.wav",
               r"CUE 3d r=640 d=\d+ \S*sig_s\.wav", r"CUE 2d \S*sig_m\.wav", r"CUE 3d r=640 d=\d+ \S*ingram_s\.wav", r"CUE 2d \S*ingram_m\.wav",
               r"CUE 3d r=640 d=\d+ \S*mosb_s1\.wav", r"CUE 2d \S*mosb_m\.wav", r"CUE 3d r=640 d=\d+ \S*m14_s\.wav", r"CUE 2d \S*m14_m\.wav",
               r"CUE 3d r=640 d=\d+ \S*hk_s\.wav", r"CUE 2d \S*hk_m\.wav", r"CUE 3d r=640 d=\d+ \S*p90_s\.wav", r"CUE 2d \S*p90_m\.wav",
               r"CUE 3d r=640 d=\d+ \S*swist\.wav", r"CUE 3d r=1280 d=\d+ \S*rock_lup\.wav"],
    "campaign": [r"CUE 3d r=640 d=\d+ \S*glock_s\.wav", r"CUE 3d r=1280 d=\d+ \S*weapons/ryko\d\.wav", r"CUE 3d r=4096 d=\d+ \S*speech/"],
    "retail": [r"CUE 2d \S*pickup/ammo\.wav", r"CUE 2d \S*pickup/sig_tke\.wav", r"CUE 3d r=640 d=\d+ \S*grt_ryko\.wav", r"CUE 3d r=4024 d=\d+ \S*enemies/\w+/dead\d\.wav"],
    "menu": [r"CUE music play menu"],
    "lever": [r"CUE 3d r=1536 d=\d+ \S*wajcha\.wav", r"CUE 3d r=640 d=\d+ \S*psss1\.wav"],
    "door": [r"CUE 3d r=640 d=\d+ \S*psss1\.wav"],
    "pickup": [r"CUE 2d \S*pickup/general\.wav"],
    "arsenal": [r"CUE 3d r=640 d=\d+ \S*%s\.wav" % name for name in ("glock_s", "s&w_s1", "sig_s", "ingram_s", "mosb_s1", "m14_s", "hk_s", "p90_s")] + [r"CUE 3d r=1280 d=\d+ \S*rock_lup\.wav"],
    "noise": [r"CUE 3d r=1280 d=\d+ \S*enemies/\w+/halt\d\.wav"],
    "revolver": [r"CUE 3d r=640 d=\d+ \S*s&w_s1\.wav", r"CUE 2d \S*s&w_m\.wav", r"CUE 3d r=640 d=\d+ \S*luska1\.wav"],
    "move": [r"CUE 2d \S*hero/skok\.wav", r"CUE 2d \S*hero/KROK"],
    "input": [r"CUE 2d \S*hero/skok\.wav"],
    "hud": [r"CUE 2d \S*mason/SERDUCHO\.WAV"],
    "gunfire": [r"CUE 3d r=640 d=\d+ \S*hero/wcialo\.wav", r"CUE 3d r=4024 d=\d+ \S*enemies/\w+/dead\d\.wav", r"CUE 3d r=1280 d=\d+ \S*weapons/ryko\d\.wav"],
    "dialogue": [r"CUE 3d r=4096 d=\d+ \S*speech/"],
    "ai-fight": [r"CUE 3d r=2048 d=\d+ \S*enemies/\w+/halt\d\.wav", r"CUE 3d r=5120 d=\d+ \S*glock_s\.wav"],
    "melee": [r"CUE 3d r=640 d=\d+ \S*swist\.wav", r"CUE 3d r=640 d=\d+ \S*palka_sciana\.wav", r"CUE 3d r=640 d=\d+ \S*weapons/wcialo\.wav", r"CUE 2d \S*weapons/wcialo\.wav"],
    "altfire": [r"CUE 3d r=640 d=\d+ \S*m14_s\.wav"],
}
# Cues that must NOT appear (retail never plays them).
NO_CUES = {}
EVERYWHERE_NO = [r"CUE \S+ (r=\d+ d=\S+ )?\S*zawleka"]  # the grenade pin sound (cshell never reads the grenade's sound_shoot)
CAPTURE_TIMES = "3"  # the run ends when the probe finishes (the capture also leaves <label>-end.png)


def default_exe():
    target = os.environ.get("CARGO_TARGET_DIR")
    return Path(target) / "debug" / "level-viewer.exe" if target else ROOT / "crates" / "level-viewer" / "target" / "debug" / "level-viewer.exe"


def build():
    print("building the optimized dev profile (first build of a target dir takes ~25 min) ...", flush=True)
    return subprocess.run(["cargo", "build", "-j", "4"] + FAST, cwd=ROOT / "crates" / "level-viewer").returncode


def times_of(row):
    return row[6] if len(row) > 6 else CAPTURE_TIMES


def command_line(row, exe, timeout, output="../../output", png="out.png"):
    label, scenario, world, extra, _, _ = row[:6]
    env = " ".join(f"{k}={v}" for k, v in extra.items()).replace("SCRATCH=1", "MESTER_USER_DIR=<empty scratch dir>")
    lead = f"MESTER_TEST_SCENARIO={scenario} " if scenario else ""
    return f"MESTER_SILENT=1 {lead}{(env + ' ') if env else ''}timeout {timeout} {exe} {world} {output} {png} {times_of(row)}"


def unregistered():
    """Scenario-looking strings in the probe sources that SCENARIOS does not know (heuristic, informational)."""
    known = {row[1] for row in SCENARIOS if row[1]}
    found = set()
    for source in (ROOT / "crates" / "level-viewer" / "src").glob("*.rs"):
        for line in source.read_text(encoding="utf-8", errors="replace").splitlines():
            if "MESTER_TEST_SCENARIO" in line or re.search(r"\bmatches!\(mode", line) or re.search(r'mode(\.as_str\(\))?\s*==\s*"', line):
                found.update(re.findall(r'"([a-z][a-z0-9_]{2,15})"', line))
    ignore = {"the", "world", "menu", "retail", "pl", "hu", "altfire|melee|flash", "fight", "far", "patrol"}
    return sorted(name for name in found - known - ignore if not name.startswith("mester"))


def run(row, exe, logs, timeout):
    label, scenario, world, extra, expect, _ = row[:6]
    png = logs / f"{label}.png"
    log = logs / f"{label}.log"
    extra = dict(extra)
    if extra.pop("SCRATCH", None):
        scratch = logs / f"{label}-user"
        shutil.rmtree(scratch, ignore_errors=True)
        scratch.mkdir(parents=True)
        extra["MESTER_USER_DIR"] = str(scratch)
    env = dict(os.environ, MESTER_SILENT="1", MESTER_CUE_LOG="1", **extra)
    env.pop("MESTER_TEST_SCENARIO", None)
    if scenario:
        env["MESTER_TEST_SCENARIO"] = scenario
    started = time.time()
    try:
        done = subprocess.run([str(exe), world, str(ROOT / "output"), str(png), times_of(row)], env=env, cwd=ROOT / "crates" / "level-viewer",
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        text, code = done.stdout.decode("utf-8", errors="replace"), done.returncode
    except subprocess.TimeoutExpired as expired:
        text, code = (expired.stdout or b"").decode("utf-8", errors="replace"), "timeout"
    text = ANSI.sub("", text)
    log.write_text(text, encoding="utf-8")
    seen = re.search(expect, text) is not None
    frames = re.search(r"PROBE VERDICT after (\d+) frames, game time ([\d.]+) s", text)
    missing = [c for c in CUES.get(label, []) if not re.search(c, text)] + ["forbidden: " + c for c in NO_CUES.get(label, []) + EVERYWHERE_NO if re.search(c, text)]
    if code == 0 and seen and missing:
        verdict = "FAIL (sound cues: %s)" % "; ".join(missing[:6])
    elif code == 0 and seen:
        verdict = "PASS"
    elif code == "timeout":
        verdict = "FAIL (timeout)"
    elif code == 0:
        verdict = "FAIL (probe never reported: no '%s' line)" % expect
    else:
        errors = [l for l in text.splitlines() if " ERROR " in l and "wgpu" not in l and "Loader Message" not in l]
        verdict = "FAIL (exit %s: %s)" % (code, errors[-1].split(" ERROR ")[-1][:200] if errors else "see log")
    return verdict, time.time() - started, (f"{frames.group(1)} frames / {frames.group(2)} s game time" if frames else "")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("names", nargs="*", help="scenario labels (default: all)")
    parser.add_argument("--exe", default=None)
    parser.add_argument("--logs", default=None)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--build", action="store_true", help="cargo build the optimized dev profile first")
    args = parser.parse_args()
    rows = [r for r in SCENARIOS if not args.names or r[0] in args.names]
    unknown = set(args.names) - {r[0] for r in SCENARIOS}
    if unknown:
        sys.exit("unknown scenario(s): " + ", ".join(sorted(unknown)))
    exe = Path(args.exe) if args.exe else default_exe()
    logs = Path(args.logs) if args.logs else Path(tempfile.gettempdir()) / "mester-probe-logs"
    logs.mkdir(parents=True, exist_ok=True)
    if args.list:
        for row in rows:
            print(f"{row[0]:10} {command_line(row, 'level-viewer.exe', args.timeout)}   # {row[5]}")
        return
    if args.build and build() != 0:
        sys.exit("build failed")
    if not exe.is_file():
        sys.exit(f"level-viewer.exe not found: {exe} (cargo build first, or --exe / CARGO_TARGET_DIR)")
    results = []
    for row in rows:
        verdict, seconds, frames = run(row, exe, logs, args.timeout)
        results.append((row[0], row[2], verdict, seconds))
        note = f"  [known open: {KNOWN_OPEN[row[0]]}]" if row[0] in KNOWN_OPEN and not verdict.startswith("PASS") else ""
        print(f"{row[0]:10} {row[2]:18} {verdict}  ({seconds:.0f} s; {frames}){note}", flush=True)
    failed = [r for r in results if not r[2].startswith("PASS")]
    print(f"\n{len(results) - len(failed)}/{len(results)} passed; logs and screenshots in {logs}")
    extra = unregistered() if not args.names else []
    if extra:
        print("unregistered scenario names found in the sources (add them to SCENARIOS if they are probes): " + ", ".join(extra))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
