"""Whole-level single-frame comparison: retail (reverse creation order) vs remake (forward order) from random states.
Reports, per level, the action sets whose same-frame outcome differs."""
import random,sys,collections
from sim import *
from cmp import randomize,state_r,state_m,names_in,norm

def fired(rt,node,W):
    ev=[];before=state_r(rt)
    rt.run(node,W,ev)
    return ev,before!=state_r(rt)

def one_frame(lv,seed,names,p):
    rng=random.Random(seed)
    rt=Retail();mk=Remake();randomize(rt,mk,rng)
    # bias flags to exercise gated actions
    for n in BOOLS:
        b=rng.random()<p
        rt.flags[n]=b;mk.flags[n]=b
    W=World(rng.random(),names)
    nodes=actions_of(lv)
    er=[];em=[];who_r=[];who_m=[]
    for n in nodes:
        e=[];rt.run(n,W,e);er+=e
        if e or False: who_r.append(n.name+'@'+str(n.line))
    racts=REMAKE[lv.name]
    for a in racts:
        e=[];mk.run(a,W,e);em+=e
        if e: who_m.append(a['name']+'@'+str(a['line']))
    return rt,mk,norm(er),norm(em),who_r,who_m

def last_dialog(ev):
    d=[e for e in ev if e[0]=='dialog']
    return d[-1][1] if d else None

if __name__=='__main__':
    N=int(sys.argv[1]) if len(sys.argv)>1 else 4000
    summary={}
    for lv in L:
        names=names_in(lv)
        cnt=collections.Counter();ex={}
        for i in range(N):
            p=[0.15,0.5,0.85][i%3]
            rt,mk,er,em,wr,wm=one_frame(lv,hash((lv.name,i))&0xffffffff,names,p)
            if sorted(map(repr,er))!=sorted(map(repr,em)) or state_r(rt)!=state_m(mk):
                kind='state' if state_r(rt)!=state_m(mk) else 'events'
                key=(kind,tuple(sorted(set(wr)^set(wm))) or tuple(sorted(set(wr))))
                cnt[key]+=1;ex.setdefault(key,(er,em,wr,wm))
            elif last_dialog(er)!=last_dialog(em):
                key=('lastdialog',tuple(sorted(set(wr))));cnt[key]+=1;ex.setdefault(key,(er,em,wr,wm))
        summary[lv.name]=(cnt,ex)
        print(lv.name,'frames with difference',sum(cnt.values()),'/',N)
        for k,c in cnt.most_common(8):
            print('   ',c,k[0],k[1]);print('       retail',ex[k][0]);print('       remake',ex[k][1])
