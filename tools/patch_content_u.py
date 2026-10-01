"""Add `content_u` to already exported *.visual.materials.json files.

Same rule as export_visual.dtx_content_u, measured on the exported PNG-s'
source DTX so existing exports need not be regenerated.
"""
import argparse,json
from pathlib import Path
from .export_visual import dtx_content_u

def patch(output:Path,game:Path):
    available={p.relative_to(game).as_posix().casefold():p for p in game.rglob('*.dtx')}
    changed=0
    for path in output.rglob('*.visual.materials.json'):
        data=json.loads(path.read_text(encoding='utf-8'))
        dirty=False
        for value in data.values():
            source=available.get(value.get('source_texture','').replace(chr(92),'/').casefold())
            content=dtx_content_u(source) if source else None
            if content is None:
                if value.pop('content_u',None) is not None: dirty=True
            elif value.get('content_u')!=content:
                value['content_u']=content;dirty=True
        if dirty:
            path.write_text(json.dumps(data,indent=2),encoding='utf-8');changed+=1
    print('patched',changed,'material files')

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('output',type=Path);ap.add_argument('game',type=Path)
    a=ap.parse_args();patch(a.output,a.game)
