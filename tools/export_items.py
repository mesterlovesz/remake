"""Read-only export of placed prison pickups and their original item values."""
import json,shutil
from pathlib import Path
from .decode_scripts import decode_text, read_script
from .export_presentation import blocks
from .export_visual import write_dtx_png
from .ltb import read_ltb

def export(game,output,worlds=('rh1-wiezienie2','rh1-wiezienie3','rh2-wiezienie1','rh2-wiezienie2')):
    catalog=blocks(read_script(game/'scripts/items.txt'),'item')
    keys={line.split(maxsplit=1)[0]:line.split(maxsplit=1)[1] for line in json.loads((output/'gameplay_scripts.json').read_text(encoding='utf-8'))['text_keys'].splitlines() if line.startswith('>') and len(line.split(maxsplit=1))==2}
    definitions=json.loads((output/'items.json').read_text(encoding='utf-8')) if (output/'items.json').exists() else {};count=0
    for world in worlds:
        scene=json.loads((output/f'{world}.scene.json').read_text(encoding='utf-8'));items=[]
        for obj in scene['objects']:
            if obj['kind']!='o_item_podnoszony':continue
            p=obj['properties'];name=p['Rodzaj_item']
            if name not in catalog:continue
            d=dict(catalog[name]);model=d.get('mesh','').replace('\\','/')
            if not model:continue
            if name not in definitions:
                target=output/f'{model}.json';target.parent.mkdir(parents=True,exist_ok=True)
                data=read_ltb(game/model);target.write_text(json.dumps(data,separators=(',',':')),encoding='utf-8')
                skins={}
                for key,value in d.items():
                    if key.startswith('tex') and key[3:].isdigit():
                        image=value.replace('\\','/');dest=f'model_textures/{image}.png';write_dtx_png(game/image,output/dest);skins[key[3:]]=dest
                sound=d.get('pickup_sound','').replace('\\','/')
                if sound and (game/sound).is_file():
                    dest=output/'audio'/sound;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(game/sound,dest)
                definitions[name]=dict(model=model,skins=skins,animation=next(iter(data['animations'])),title=keys.get(d.get('title',''),name),health=float(d.get('health',0)),ammo=int(d.get('amount',d.get('ammo_amount',0))),ammo_for=int(d.get('ammo_for',d.get('ammo_index',-1))),weapon='weapon' in d,scale=float(d.get('scale',1)),sound=sound,commands=catalog[name])
            items.append(dict(name=p['Name'],kind=name,pos=p['Pos'],rotation=p['Rotation']))
        count+=len(items);(output/f'{world}.items.json').write_text(json.dumps(items,separators=(',',':')),encoding='utf-8')
    (output/'items.json').write_text(json.dumps(definitions,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    print(f'{count} original pickups, {len(definitions)} definitions')
if __name__=='__main__':export(Path('../GYARI'),Path('output'))
