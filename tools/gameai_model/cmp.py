import random,sys,collections
from sim import *

def norm(ev):
    out=[]
    for e in ev:
        if e[0] in('hostileattack',) and e in out: continue
        out.append(e)
    return out
def state_r(r): return tuple(sorted((k,v) for k,v in r.flags.items() if k in declared and declared[k]['bool']))+tuple(sorted((k,round(v,3)) for k,v in r.ctr.items()))
def state_m(m):
    return tuple(sorted((k,m.flag(k)) for k in BOOLS))+tuple(sorted((k,round(v,3)) for k,v in m.timers.items()))
def names_in(level):
    ns=set()
    for n in level.nodes:
        for k,v in n.f.items():
            if k in('ifalive','ifdead','ifaction','ifplayerseenby'): ns.add(v)
            if k in('ifgraczblizejniz',): ns.add(v[1])
    return sorted(ns)
def randomize(rt,mk,rng):
    for n in BOOLS:
        b=rng.random()<0.5
        rt.flags[n]=b; mk.flags[n]=b
    for n in COUNTERS:
        v=rng.choice([0.0,1.0,10.0,21.0,31.0,100.0])
        rt.ctr[n]=v; mk.timers[n]=v

def per_action(N=3000):
    res={}
    for lv in L:
        racts={a['line']:a for a in REMAKE[lv.name]}
        nodes=actions_of(lv)
        names=names_in(lv)
        for node in nodes:
            ract=racts[node.line]
            diffs=collections.Counter();ex={}
            for i in range(N):
                rng=random.Random(hash((lv.name,node.line,i)) & 0xffffffff)
                rt=Retail();mk=Remake();randomize(rt,mk,rng)
                W=World(rng.random(),names)
                ev_r=[];ev_m=[]
                rt.run(node,W,ev_r);mk.run(ract,W,ev_m)
                ev_r=norm(ev_r);ev_m=norm(ev_m)
                sr=state_r(rt);sm=state_m(mk)
                if ev_r!=ev_m:
                    kind='events-order' if sorted(map(repr,ev_r))==sorted(map(repr,ev_m)) else 'events'
                    diffs[kind]+=1;ex.setdefault(kind,(ev_r,ev_m))
                if sr!=sm:
                    diffs['state']+=1;ex.setdefault('state',None)
            res[(lv.name,node.name,node.line)]=(dict(diffs),ex)
    return res

if __name__=='__main__':
    res=per_action(int(sys.argv[1]) if len(sys.argv)>1 else 1500)
    bad={k:v for k,v in res.items() if v[0]}
    print('actions checked',len(res),'differing',len(bad))
    for k,v in bad.items(): print(k,v)
