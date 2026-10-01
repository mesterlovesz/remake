"""Export named DAT world models (doors, bars and glass) separately."""
import argparse,json
from pathlib import Path
from .render_dat import read_model_render_nodes
from .export_visual import export_visual
from .lithtech_dat import read_world

def export(source,game,output):
    prefix=f'world_models/{source.stem}/'
    models=read_model_render_nodes(source)
    objects={o.properties.get('Name','').casefold():o for o in read_world(source).objects}
    available={p.relative_to(game).as_posix().casefold():p for p in game.rglob('*.dtx')}
    for name,nodes in models.items():
        if not name or any(c in name for c in '/\\:'):raise ValueError('invalid model filename')
        export_visual(Path(name+'.dat'),game,output/prefix,nodes,prefix,
                      model_object=objects.get(name.casefold()),available=available)
    (output/f'{source.stem}.render_models.json').write_text(json.dumps(list(models)),encoding='utf-8')
    print(source.stem,len(models),'separate world models')

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('source',type=Path);ap.add_argument('game',type=Path);ap.add_argument('output',type=Path)
    args=ap.parse_args();export(args.source,args.game,args.output)
