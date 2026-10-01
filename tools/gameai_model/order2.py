import random,sys,collections
from sim import *
from cmp import randomize,state_r,state_m,names_in,norm
N=int(sys.argv[1]) if len(sys.argv)>1 else 3000
tot=0;bad=0;badorder=0
for lv in L:
    names=names_in(lv);racts=REMAKE[lv.name]
    fwd=[n for n in lv.nodes if n.name]
    for i in range(N):
        rng=random.Random(hash((lv.name,'o2',i))&0xffffffff)
        rt=Retail();mk=Remake();randomize(rt,mk,rng)
        p=[0.15,0.5,0.85][i%3]
        for n in BOOLS:
            b=rng.random()<p;rt.flags[n]=b;mk.flags[n]=b
        W=World(rng.random(),names)
        er=[];em=[]
        for n in fwd: rt.run(n,W,er)
        for a in racts: mk.run(a,W,em)
        er=norm(er);em=norm(em);tot+=1
        if state_r(rt)!=state_m(mk) or sorted(map(repr,er))!=sorted(map(repr,em)): bad+=1
        elif er!=em: badorder+=1
print('frames',tot,'retail-forward vs remake: set/state differences',bad,'event-order-only differences',badorder)
