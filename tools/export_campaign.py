"""Export campaign content and source evidence without changing retail files."""
from pathlib import Path
import argparse, json, shutil
from .decode_scripts import read_script
from .export_gameplay import export as gameplay, records, sections, text_key_map, write_json
from .export_items import export as items
from .export_props import export as props
from .export_presentation import opening_timeline, blocks
from .export_visual import write_dtx_png
from .ltb import read_ltb

ORDER = ('rh3-miasteczko0','rh1-wiezienie1','rh1-wiezienie2','rh1-wiezienie3','rh2-wiezienie1','rh2-wiezienie2','rh3-miasteczko1','rh3-miasteczko2','burmistrz1','burmistrz2','chapel_mniejszy','knajpa','Rh7a-Tunele','podziemia1','podziemia1a','podziemia1b','podziemia1c','chinatown2','RH9-fabryka','rh10-wiezowiec1','rh10-wiezowiec2','rh10-wiezowiec3','wiez_wn1','wiez_wn2','wiez_wn3','rh12-lab1','rh12-lab2')
# Chinatown's first world belongs in the authored campaign inventory even though
# its DAT version is unsupported; transition evidence is retained separately.
CAMPAIGN = (*ORDER, 'chinatown')

def cutscene_assets(game, output):
    catalog=blocks(read_script(game/'scripts/postacie.txt'),'postac')
    actors=json.loads((output/'actors.json').read_text(encoding='utf-8')) if (output/'actors.json').exists() else {}
    scenes=blocks(read_script(game/'scripts/scenki.txt'),'scena')
    timelines={name:opening_timeline(game,name) for name in scenes}
    timeline=[phase for phases in timelines.values() for phase in phases]
    models={p['model'] for p in timeline}
    for phase in timeline:
        for key,kind in phase['settings'].items():
            if not key.startswith('postac') or kind in actors:continue
            lines=catalog[kind];split=next((i for i,(k,v) in enumerate(lines) if k=='faza'),len(lines));d=dict(lines[:split]);skins={}
            for k,v in d.items():
                if k.startswith('skin') and k[4:].isdigit():
                    texture=v.replace(chr(92),'/');target=f'model_textures/{texture}.png';write_dtx_png(game/texture,output/target);skins[k[4:]]=target
            phases=blocks('\n'.join(f'{k} {v}' for k,v in lines[split:]),'faza')
            actors[kind]={'model':d['model'].replace(chr(92),'/'),'skins':skins,'styles':{k[2:]:v for k,v in d.items() if k.startswith('rs') and k[2:].isdigit()},'default':d.get('default_faza',''),'animations':{k:dict(v).get('animacja',k) for k,v in phases.items()}}
            models.add(actors[kind]['model'])
        for field in ('sound','speech'):
            if phase[field]:
                target=output/'audio'/phase[field];target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(game/phase[field],target)
    for model in models:write_json(output/f'{model}.json',read_ltb(game/model))
    write_json(output/'actors.json',actors);write_json(output/'bus_escape.json',timelines['ucieczka z 2 wiezienia']);write_json(output/'campaign_cutscenes.json',timelines)

def inventory(game,output):
    ai=read_script(game/'scripts/ai/gameai.txt');keys=text_key_map(read_script(game/'scripts/text_keys.txt'))
    levels={g['name'].replace(chr(92),'/').removeprefix('worlds/').lower():g for g in sections(records(ai),'level')[1]}
    rows=[]
    for world in CAMPAIGN:
        g=levels[world.lower()];commands=g['records'];title=next((v for k,v,_ in commands if k=='load_d'),'');scene_path=output/f'{world}.scene.json';links=[]
        if scene_path.exists():
            for obj in json.loads(scene_path.read_text(encoding='utf-8'))['objects']:
                for key,value in obj['properties'].items():
                    if isinstance(value,str) and value.lower().startswith('worlds'+chr(92)):links.append({'kind':'object','object':obj['properties'].get('Name'),'property':key,'target':value})
        links += [{'kind':'script','source_line':line,'target':value} for key,value,line in commands if key=='startlevel']
        rows.append({'world':world,'playable':world!='rh1-wiezienie1','linked_main_route':world!='chinatown','title':keys.get(title,title),'ai_source_line':g['source_line'],'scene_available':scene_path.exists(),'gameplay_available':(output/f'{world}.gameplay.json').exists(),'links':links})
    write_json(output/'campaign-inventory.json',{'format':'mesterlovesz-campaign-inventory-v1','playable_maps':27,'linked_playable_maps':26,'legacy_unlinked_gameplay_worlds':['chinatown'],'cinematic_campaign_maps':1,'shipped_dat_files':31,'extra_shipped_worlds':['nic','katscena','outro'],'unshipped_test_blocks':['test2','pudlo','test','boks'],'worlds':rows})

def export(game,output,worlds):
    gameplay(game,output,worlds)
    items(game,output,worlds)
    for world in worlds:props(game/'worlds'/f'{world}.dat',game,output)
    cutscene_assets(game,output)
    inventory(game,output)

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('game',type=Path);ap.add_argument('output',type=Path);ap.add_argument('--worlds',nargs='+',default=['rh3-miasteczko0','rh3-miasteczko1','rh3-miasteczko2','burmistrz1','burmistrz2']);a=ap.parse_args();export(a.game,a.output,a.worlds)
