"""Export authored environment props with their retail default pose and skins."""
from pathlib import Path
import argparse,json,struct
from .export_presentation import blocks
from .decode_scripts import decode_text, read_script
from .ltb import read_ltb
from .export_visual import write_dtx_png

def export(source,game,output):
    scene=json.loads((output/f'{source.stem}.scene.json').read_text(encoding='utf-8'))
    catalog=blocks(read_script(game/'scripts/objects.txt'),'object')
    definitions={}
    for name,lines in catalog.items():
        d=dict(lines);d['definition_name']=name
        if 'model' in d:definitions.setdefault(d['model'].replace('\\','/').casefold(),d)
    props=[];loaded=set();missing=[]
    for entity in scene['objects']:
        if entity['kind']!='o_obiekt':continue
        e=entity['properties'];path=e['Model'].replace('\\','/');d=definitions.get(path.casefold())
        if d is None:missing.append(path);continue
        if path.casefold() not in loaded:
            m=read_ltb(game/path)
            target=output/f'{path}.json';target.parent.mkdir(parents=True,exist_ok=True)
            target.write_text(json.dumps(m,separators=(',',':')),encoding='utf-8');loaded.add(path.casefold())
        skins={}
        for k,v in d.items():
            if k.startswith('skin') and k[4:].isdigit():
                texture=v.replace(chr(92),'/')
                if texture.lower().endswith('.spr'):
                    raw=(game/texture).read_bytes();size=struct.unpack_from('<H',raw,20)[0]
                    texture=raw[22:22+size].decode('cp1250').replace(chr(92),'/')
                target=f'model_textures/{texture}.png'
                write_dtx_png(game/texture,output/target);skins[k[4:]]=target
        props.append(dict(name=e['Name'],definition_name=d['definition_name'],model=path,pos=e['Pos'],rotation=e['Rotation'],animation=d.get('anim0',d.get('anim','base')),
            skins=skins,styles={k[2:]:v for k,v in d.items() if k.startswith('rs') and k[2:].isdigit()}))
    (output/f'{source.stem}.props.json').write_text(json.dumps(props,separators=(',',':')),encoding='utf-8')
    print(source.stem,len(props),'props; unresolved definitions:',sorted(set(missing)))

DEBRIS_MODELS=('ceramika01','ceramika02','ceramika03','ceramika04','blacha01','blacha02','blacha03','blacha04','blacha05','prety01','prety02','prety03','prety04','prety05',
    'deski_polamane01','deski_polamane02','deski_polamane03','deski_polamane04','deski_polamane05','gruz01','gruz02','gruz03','gruz04','drewienko01','drewienko02','drewienko03','papier01','papier02','papier03')
DEBRIS_SKINS={'ceramika':'skins/levelowe/kawalki/ceramika.dtx','blacha':'skins/levelowe/kawalki/blacha.dtx','prety':'skins/levelowe/kawalki/prety.dtx','deski_polamane':'skins/levelowe/kawalki/deski.dtx',
    'gruz':'skins/levelowe/kawalki/gruz.dtx','drewienko':'skins/levelowe/kawalki/deski.dtx','papier':'skins/levelowe/kartki.dtx'}

def skin_frames(game,output,texture):
    """PNG paths of one objects.txt skin: a DTX is one frame, a .spr (the burning wrecks' flame) lists its frames and rate."""
    texture=texture.replace(chr(92),'/')
    if not texture.lower().endswith('.spr'):
        target=f'model_textures/{texture}.png';write_dtx_png(game/texture,output/target);return [target],0
    raw=(game/texture).read_bytes();count,fps=struct.unpack_from('<2I',raw);offset=20;frames=[]
    for _ in range(count):
        size=struct.unpack_from('<H',raw,offset)[0];offset+=2
        frame=raw[offset:offset+size].decode('cp1250').replace(chr(92),'/');offset+=size
        target=f'model_textures/{frame}.png';write_dtx_png(game/frame,output/target);frames.append(target)
    return frames,fps

def export_definitions(game,output):
    """Every objects.txt definition (death_podmien wrecks included) plus the `kawalki` debris models, into props_defs.json.
    Additive: existing per-level props.json files are untouched."""
    catalog=blocks(read_script(game/'scripts/objects.txt'),'object');definitions={};models=set()
    for name,lines in catalog.items():
        d=dict(lines)
        if 'model' not in d:continue
        path=d['model'].replace(chr(92),'/')
        if not (game/path).is_file():continue
        if path.casefold() not in models:
            target=output/f'{path}.json';target.parent.mkdir(parents=True,exist_ok=True)
            target.write_text(json.dumps(read_ltb(game/path),separators=(',',':')),encoding='utf-8');models.add(path.casefold())
        entry=dict(model=path,skins={},swap_skins={},fps={})
        for key,value in d.items():
            for prefix,slot in (('skin','skins'),('podmieniany_skin','swap_skins')):
                if key.startswith(prefix) and key[len(prefix):].isdigit() and (game/value.replace(chr(92),'/')).is_file():
                    frames,fps=skin_frames(game,output,value);entry[slot][key[len(prefix):]]=frames
                    if fps:entry['fps'][key[len(prefix):]]=fps
        definitions[name]=entry
    debris={}
    for stem in DEBRIS_MODELS:
        path=f'models/levelowe/kawalki/{stem}.ltb';target=output/f'{path}.json'
        target.parent.mkdir(parents=True,exist_ok=True);target.write_text(json.dumps(read_ltb(game/path),separators=(',',':')),encoding='utf-8')
        skin=DEBRIS_SKINS[stem.rstrip('0123456789').rstrip('_')];frames,_=skin_frames(game,output,skin);debris[path]=frames[0]
    (output/'props_defs.json').write_text(json.dumps(dict(definitions=definitions,debris=debris),separators=(',',':')),encoding='utf-8')
    print(len(definitions),'object definitions,',len(debris),'debris models')

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('source',type=Path);ap.add_argument('game',type=Path);ap.add_argument('output',type=Path)
    args=ap.parse_args()
    if args.source.name=='definitions':export_definitions(args.game,args.output)
    else:export(args.source,args.game,args.output)
