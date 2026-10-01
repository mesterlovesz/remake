"""Export the retail HUD, PCM audio and opening scene timeline, without editing inputs."""
from pathlib import Path
import argparse, json, shutil
from .decode_scripts import decode_text, read_script
from .export_visual import write_dtx_png
from .ltb import read_ltb

def blocks(text, keyword):
    result={}; current=None
    for line in text.splitlines():
        line=line.strip()
        if not line or line.startswith('//'): continue
        key,_,value=line.partition(' ')
        if key==keyword:
            current=[];result[value.strip()]=current
        elif current is not None: current.append((key,value.strip()))
    return result

def opening_timeline(game, scene='intro'):
    text=read_script(game/'scripts/scenki.txt')
    scenes=blocks(text,'scena')
    keys=dict(line.strip().split(' ',1) for line in read_script(game/'scripts/text_keys.txt').splitlines() if line.startswith('>') and ' ' in line)
    phase_id=None;elapsed=0.0;result=[];visited=set()
    while True:
        lines=scenes[scene]
        split=next(i for i,(k,v) in enumerate(lines) if k=='faza')
        header=dict(lines[:split])
        phases=blocks('\n'.join(f'{k} {v}' for k,v in lines[split:]),'faza')
        phase_id=phase_id or header['pierwsza_faza']
        if (scene,phase_id) in visited: raise ValueError('cycle in opening')
        visited.add((scene,phase_id))
        phase=dict(phases[phase_id])
        if 'cutscene' in phase: scene=phase['cutscene'];phase_id=None;continue
        duration=float(phase.get('length',0)) if 'runworld' not in phase else 0.0
        entry={'scene':scene,'id':phase_id,'start':elapsed,'duration':duration,'model':header['model'].replace('\\','/'),
            'settings':header,'phase':phase,'sound':phase.get('glos','').replace('\\','/'),
            'speech':keys.get(phase.get('gadka',''),'').replace('\\','/'),'subtitle':keys.get(phase.get('subtitle',''),''),
            'runworld':phase.get('runworld','').replace('\\','/').removeprefix('worlds/')}
        result.append(entry)
        if entry['runworld']: break
        elapsed+=duration
        phase_id=phase.get('nast_faza')
        if phase_id is None: break
    return result

def export(game, output):
    output.mkdir(parents=True,exist_ok=True)
    for p in (game/'misc/panel_l/HUD').glob('*.dtx'):
        write_dtx_png(p,output/'hud'/f'{p.stem}.png')
    for p in (game/'misc/panel/ammo/fonts').glob('*.dtx'):
        write_dtx_png(p,output/'hud'/f'{p.stem}.png')
    write_dtx_png(game/'textures/sprajty/system/celownik.dtx',output/'hud/celownik.png')
    shutil.copyfile(Path('C:/Windows/Fonts/consolab.ttf'),output/'hud/subtitles.ttf')
    timeline=opening_timeline(game)
    characters=blocks(read_script(game/'scripts/postacie.txt'),'postac')
    model_paths=set(p['model'] for p in timeline)
    actors={}
    for phase in timeline:
        for key,kind in phase['settings'].items():
            if key.startswith('postac') and kind not in actors:
                lines=characters[kind]
                split=next((i for i,(k,v) in enumerate(lines) if k=='faza'),len(lines))
                definition=dict(lines[:split]);skins={}
                for k,v in definition.items():
                    if k.startswith('skin') and k[4:].isdigit():
                        target=f'model_textures/{v.replace(chr(92),"/")}.png'
                        write_dtx_png(game/v.replace('\\','/'),output/target)
                        skins[k[4:]]=target
                animation_phases=blocks('\n'.join(f'{k} {v}' for k,v in lines[split:]),'faza')
                actors[kind]={'model':definition['model'].replace('\\','/'),'skins':skins,
                    'styles':{k[2:]:v for k,v in definition.items() if k.startswith('rs') and k[2:].isdigit()},
                    'default':definition.get('default_faza',''),
                    'animations':{k:dict(v).get('animacja',k) for k,v in animation_phases.items()}}
                model_paths.add(actors[kind]['model'])
    for path in sorted(model_paths):
        model=read_ltb(game/path)
        target=output/f'{path}.json';target.parent.mkdir(parents=True,exist_ok=True)
        target.write_text(json.dumps(model,separators=(',',':')),encoding='utf-8')
        print(f'Model: {path}, {len(model["nodes"])} bones, {len(model["animations"])} animations')
    (output/'actors.json').write_text(json.dumps(actors,indent=2),encoding='utf-8')
    (output/'opening.json').write_text(json.dumps(timeline,ensure_ascii=False,indent=2),encoding='utf-8')
    # Footsteps and the fall-damage cry (cshell 0x1006105d).
    paths={f'sounds/hero/KROK{i}.WAV' for i in (1,2)}|{'sounds/speech/hero/spad.wav'}
    for phase in timeline:
        for key in ('sound','speech'):
            if phase.get(key): paths.add(phase[key])
    for path in paths:
        source=game/path.replace('\\','/')
        target=output/'audio'/path.replace('\\','/')
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(source,target)
    print(f'Exported HUD, {len(paths)} audio files, {len(timeline)} opening phases')

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('game',type=Path);ap.add_argument('output',type=Path)
    args=ap.parse_args();export(args.game,args.output)
