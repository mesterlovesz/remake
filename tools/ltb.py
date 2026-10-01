"""Checked reader for the retail PC LTB v9/model v23 layout.

Field layout cross-checked with haekb/io_scene_lithtech (research only).
Outputs native coordinates, XYZW quaternions, row-major bind matrices.
"""
from pathlib import Path
import struct

class Reader:
    def __init__(self,data): self.data=data;self.offset=0
    def take(self,size):
        if size<0 or self.offset+size>len(self.data): raise ValueError(f'LTB truncated at {self.offset:#x}, need {size}')
        value=self.data[self.offset:self.offset+size];self.offset+=size;return value
    def values(self,fmt): return list(struct.unpack('<'+fmt,self.take(struct.calcsize('<'+fmt))))
    def value(self,fmt='I'): return self.values(fmt)[0]
    def string(self): return self.take(self.value('H')).decode('cp1250')
    def count(self):
        n=self.value()
        if n>1000000: raise ValueError(f'invalid LTB count {n} at {self.offset-4:#x}')
        return n

def mesh(r):
    textures=r.values('5I');style=r.value();priority=r.value('b');kind=r.value()
    # `style` is the piece's render style index: objects.txt `rsN` (rs\*.ltb) N, which is NOT the texture slot (samochod_niebieski_bucha: flames use texture 2, style 0).
    result={'texture':textures[1],'style':style,'positions':[],'uvs':[],'normals':[],'weights':[],'indices':[]}
    if kind==7: r.take(4)
    else:
        size=r.value();nv=r.value()
        if kind==6: unique=r.value()
        nf,nbf,nbv=r.values('3I')
        if nv>1000000 or nf>1000000: raise ValueError('invalid mesh size')
        if kind not in (4,5,6): raise ValueError(f'unsupported mesh kind {kind}')
        if kind==5: reindexed=r.value('B')
        masks=r.values('4I')
        bone=r.value() if kind==4 else None
        if kind==6:
            result['morph_node']=r.value();bone=r.value()
        if kind==5:
            palette=r.value('B')
            if palette or reindexed: raise ValueError('unsupported indexed bone palette')
        positions=[[0,0,0] for _ in range(nv)];normals=[[0,1,0] for _ in range(nv)];uvs=[[0,0] for _ in range(nv)]
        weights=[[] for _ in range(nv)]
        for mask in masks:
            if mask & ~0x1f7: raise ValueError(f'unknown vertex stream {mask:#x}')
            if not mask: continue
            for i in range(nv):
                if mask&1:
                    positions[i]=r.values('3f')
                    if kind in (4,6): weights[i]=[[bone,1.0]]
                    else:
                        blends=r.values(f'{max(0,nbf-1)}f')
                        weights[i]=[[0,b] for b in blends+[1.0-sum(blends)]]
                if mask&2: normals[i]=r.values('3f')
                if mask&4: r.take(4)
                if mask&0x10: uvs[i]=r.values('2f')
                for flag in (0x20,0x40,0x80):
                    if mask&flag:r.take(8)
                if mask&0x100:r.take(24)
        indices=r.values(f'{nf*3}H')
        if any(i>=nv for i in indices): raise ValueError('face index out of bounds')
        if kind==5:
            for _ in range(r.count()):
                start,count=r.values('2H');bones=r.values('4B');r.take(4)
                if start+count>nv: raise ValueError('bone set out of bounds')
                for i in range(start,start+count):
                    weights[i]=[[b,weights[i][j][1]] for j,b in enumerate(bones) if b!=255]
        if kind==6:
            result['duplicates']=[r.values('2H') for _ in range(r.count())]
        result.update(positions=positions,normals=normals,uvs=uvs,weights=weights,indices=indices)
    r.take(r.value('B'))
    return result

def read_ltb(path):
    r=Reader(Path(path).read_bytes())
    if r.values('2H')!=[1,9]:raise ValueError('unsupported LTB container')
    r.take(16);version=r.value()
    if version not in (23,24,25):raise ValueError(f'unsupported model version {version}')
    counts=r.values('15I');node_count=counts[2]
    command=r.string();radius=r.value('f');r.take(r.count()*(64 if version==23 else 68))
    pieces=[]
    for _ in range(r.count()):
        name=r.string();lods=r.count();r.take(lods*4+8)
        for i in range(lods):
            part=mesh(r)
            if i==0: part['name']=name;pieces.append(part)
    nodes=[]
    def node(parent):
        index=len(nodes)
        name=r.string();number=r.value('H');flags=r.value('B');bind=r.values('16f');children=r.count()
        if number!=index:raise ValueError('unexpected skeleton index')
        nodes.append(dict(name=name,parent=parent,bind=bind))
        for _ in range(children):node(index)
    node(-1)
    if len(nodes)!=node_count:raise ValueError('skeleton count mismatch')
    for _ in range(r.count()):r.string();r.take(r.count()*4)
    children=[r.string() for _ in range(max(0,r.count()-1))]
    animations={}
    for _ in range(r.count()):
        extents=r.values('3f');name=r.string();compression=r.value();blend=r.value();keys=r.count()
        times=[];events=[]
        for _ in range(keys):times.append(r.value()/1000.0);events.append(r.string())
        tracks=[]
        for _ in range(node_count):
            morph=[]
            if compression==0:
                if r.value('B')!=0:
                    morph=[[r.values('3f') for _ in range(r.count())] for _ in range(keys)]
                    pos=[[0,0,0]];rot=[[0,0,0,1]]
                else:
                    pos=[r.values('3f') for _ in range(keys)];rot=[r.values('4f') for _ in range(keys)]
            elif compression in (1,2,3):
                pos=[r.values('3h' if compression==2 else '3f') for _ in range(r.count())]
                rot=[r.values('4f' if compression==1 else '4h') for _ in range(r.count())]
                if compression==2:pos=[[v/16.0 for v in p] for p in pos]
                if compression!=1:rot=[[v/32767.0 for v in q] for q in rot]
            else:raise ValueError(f'unknown animation compression {compression}')
            if not pos or not rot:raise ValueError('empty animation track')
            tracks.append(dict(pos=pos,rot=rot,morph=morph))
        animations[name]=dict(times=times,events=events,tracks=tracks,translation=[0.0,0.0,0.0],dimensions=extents)
    sockets={}
    for _ in range(r.count()):
        bone=r.value();name=r.string();rot=r.values('4f');pos=r.values('3f');scale=r.values('3f')
        sockets[name]=dict(bone=bone,pos=pos,rot=rot,scale=scale)
    # These bindings follow the sockets. LithTech adds the per-animation
    # translation to the root node before composing its children's transforms.
    # There is one binding group for this model and one for each child model.
    bindings=[]
    for child in [None]+children:
        group={}
        for _ in range(r.count()):
            name=r.string();dimensions=r.values('3f');translation=r.values('3f')
            group[name]=dict(dimensions=dimensions,translation=translation)
            if child is None and name in animations:
                animations[name].update(group[name])
        bindings.append(dict(child=child,animations=group))
    return dict(version=version,command=command,radius=radius,pieces=pieces,nodes=nodes,animations=animations,sockets=sockets,children=children,bindings=bindings)
