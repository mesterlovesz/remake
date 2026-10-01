"""Add the per-node `graph` index to already exported *.gameplay.json navigation.

Same rule as export_gameplay.navigation. A .pth file holds several independent
`path` graphs whose place ids restart, so nodes are keyed by (graph, id). Only
the new field is added; the existing export is verified node by node first.
"""
import argparse,json
from pathlib import Path
from .export_gameplay import navigation

def patch(output:Path,game:Path):
    changed=0
    for path in sorted(output.glob('*.gameplay.json')):
        world=path.name[:-len('.gameplay.json')]
        data=json.loads(path.read_text(encoding='utf-8'))
        nodes=(data.get('navigation') or {}).get('nodes')
        source=navigation(game/'worlds'/f'{world}.pth')
        if not nodes or source is None:continue
        if len(nodes)!=len(source['nodes']) or any(a['id']!=b['id'] or a['pos']!=b['pos'] for a,b in zip(nodes,source['nodes'])):
            raise ValueError(f'{world}: exported navigation differs from {world}.pth')
        if all(a.get('graph')==b['graph'] for a,b in zip(nodes,source['nodes'])):continue
        for a,b in zip(nodes,source['nodes']):a['graph']=b['graph']
        path.write_text(json.dumps(data,ensure_ascii=False,allow_nan=False,separators=(',',':'))+'\n',encoding='utf-8');changed+=1
        print(world,len(nodes),'nodes in',source['nodes'][-1]['graph']+1,'graphs')
    print('patched',changed,'gameplay files')

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('output',type=Path);ap.add_argument('game',type=Path)
    a=ap.parse_args();patch(a.output,a.game)
