import random,re,collections,sys
from retail import *
t=open(SCRIPT,encoding='cp1250').read()
L,V,log=parse(t)
declared={v['name']:v for v in V}
COUNTERS={v['name'] for v in V if v['ctr']}
BOOLS={v['name'] for v in V if v['bool']}
# ---- dialogue nodes: retail keeps one value per key (set/unset/hostileattack)
dl=open(str(ROOT/'output'/'decoded_scripts'/'dialogi.txt'),encoding='cp1250').read()
DIALOG={};DIALOG_ORDER={};cur=None
for ln,line in enumerate(dl.replace('\r','').split('\n'),1):
    w=first_word(line);r=rest(line)
    if w=='dialog': cur=r;DIALOG.setdefault(cur,{});DIALOG_ORDER[cur]=[]
    elif cur and w in('set','unset','hostileattack'):
        DIALOG[cur][w]=r; DIALOG_ORDER[cur].append((w,r))
def remake_dialog_effects(d):
    cmds=DIALOG_ORDER.get(d,[]);out=[]
    for i,(k,v) in enumerate(cmds):
        if any(c[0]==k for c in cmds[i+1:]): continue
        out.append((k,v))
    return out
# ---- world oracle (shared by both engines)
class World:
    def __init__(s,seed,names):
        s.r=random.Random(seed);s.c={};s.names=names
        s.exist={n:s.r.random()<0.9 for n in names}
    def get(s,key,p=0.5):
        if key not in s.c: s.c[key]=s.r.random()<p
        return s.c[key]
    def dist(s,n):
        k=('dist',n)
        if k not in s.c: s.c[k]=s.r.random()*1200
        return s.c[k]
    def alive(s,n): return s.exist.get(n,True) and s.get(('alive',n),0.6)
    def exists(s,n): return s.exist.get(n,True)
def pred(W,kind,arg):
    if kind=='ifseenbyhostile': return W.get(('seenbyhostile',),0.3)
    if kind=='ifactionhostile': return W.get(('actionhostile',),0.3)
    if kind=='ifweapondrawn': return W.get(('drawn',),0.5)
    if kind=='ifweaponhidden': return not W.get(('drawn',),0.5)
    if kind=='ifhostileblizejniz': return W.get(('hostileclose',float(arg)),0.5)
    if kind=='ifplayerseenby': return W.exists(arg) and W.get(('seen',arg),0.5) and W.alive(arg)
    if kind=='ifplayerhas': return W.get(('has',arg),0.5)
    if kind=='ifaction': return W.get(('action',arg),0.3) and W.alive(arg)
    if kind=='ifalive': return W.exists(arg) and W.alive(arg)
    if kind=='ifdead': return W.exists(arg) and not W.alive(arg)
    raise KeyError(kind)
# ---------------- retail engine (cshell 0x1001a140)
class Retail:
    def __init__(s):
        s.flags={};s.ctr={}
        for n,v in declared.items():
            if v['bool']: s.flags[n]=False
            if v['ctr']: s.ctr[n]=0.0
    def holds(s,fld,val,W):
        if fld=='if_var': return (val in s.flags and s.flags[val]) if val else None
        if fld=='ifnot_var': return (val in s.flags and not s.flags[val]) if val else None
        if fld=='iflicznikwiekszyniz':
            th,name=val
            if not th>0.001: return None
            return (name in s.ctr and s.ctr[name]>th)
        if fld=='ifgraczblizejniz':
            th,name=val
            if not th>1.0: return None
            return W.exists(name) and W.dist(name)<th
        if fld=='ifhostileblizejniz':
            if not val>1.0: return None
            return pred(W,fld,val)
        if fld in('ifalive','ifdead','ifaction','ifplayerseenby','ifplayerhas'):
            if not val: return None
            return pred(W,fld,val)
        return pred(W,fld,None)
    def run(s,n,W,ev):
        while True:
            adv=False
            for fld,val in n.f.items():
                if fld=='_lines' or fld in EFFECT: continue
                h=s.holds(fld,val,W)
                if h and n.next is not None: n=n.next;adv=True;break
            if not adv: break
        f=n.f
        for k in EXEC_ORDER:
            if k not in f: continue
            v=f[k]
            if k=='dialog':
                ev.append(('dialog',v))
                d=DIALOG.get(v,{})
                if 'set' in d: s.do_set(d['set'])
                if 'unset' in d: s.do_unset(d['unset'])
                if 'hostileattack' in d: ev.append(('hostileattack-d',))
            elif k=='set': s.do_set(v)
            elif k=='unset': s.do_unset(v)
            elif k in('hostileattack','tnijitems','outro'): ev.append((k,))
            elif k=='zdrowie':
                if v!=0.0: ev.append((k,v))
            else: ev.append((k,v))
    def do_set(s,name):
        if name in s.flags: s.flags[name]=True
        elif name in s.ctr: s.ctr[name]=0.0
    def do_unset(s,name):
        if name in s.flags: s.flags[name]=False
        elif name in s.ctr: s.ctr[name]=0.0
    def tick(s,dt,W,ev,nodes):
        for k in s.ctr: s.ctr[k]+=dt
        for n in nodes: s.run(n,W,ev)
# ---------------- remake engine (mission-runtime/src/lib.rs)
def remake_actions(text):
    res={};lvl=None;cur=None
    meta={'muza','load_c','load_i','load_o','load_w','load_d','gestosc_sciezek','nie_sprawdzaj_drzwi','pure_shooter','blyski_postaci','bool','licznik'}
    for ln,line in enumerate(text.split('\n'),1):
        x=line.replace('\r','').split('//')[0].strip()
        if not x: continue
        p=x.split(None,1);k=p[0];v=p[1].strip() if len(p)>1 else ''
        if k=='level': lvl=v;res.setdefault(lvl,[]);cur=None;continue
        if lvl is None: continue
        if k=='action': cur={'name':v,'line':ln,'cmds':[]};res[lvl].append(cur);continue
        if k in meta: continue
        if cur is not None: cur['cmds'].append((ln,k,v))
    return res
REMAKE=remake_actions(t)
def number_target(v):
    p=v.split(None,1)
    try: n=float(p[0])
    except Exception: return None
    if n<0: return None
    return (n,p[1].strip() if len(p)>1 else '')
class Remake:
    def __init__(s):
        s.flags={};s.timers={n:0.0 for n in COUNTERS}
    def flag(s,n): return s.flags.get(n,False)
    def cond(s,k,v,W):
        if k=='if': return s.flag(v)
        if k=='ifnot': return not s.flag(v)
        if k in('ifweapondrawn','ifweaponhidden','ifseenbyhostile','ifactionhostile'): return pred(W,k,None)
        if k in('ifalive','ifdead','ifplayerseenby','ifaction','ifplayerhas'): return pred(W,k,v)
        if k=='ifhostileblizejniz':
            try: d=float(v)
            except Exception: return False
            return d>=0 and pred(W,k,v)
        if k=='ifgraczblizejniz':
            nt=number_target(v)
            return bool(nt) and nt[0]>1.0 and W.exists(nt[1]) and W.dist(nt[1])<nt[0]
        if k=='iflicznikwiekszyniz':
            nt=number_target(v)
            return bool(nt) and s.timers.get(nt[1],0.0)>nt[0]
        return False
    def set_flag(s,n,val):
        if n in s.timers: s.timers[n]=0.0;return
        s.flags[n]=val
    def execute(s,k,v,ev):
        if k=='set': s.set_flag(v,True)
        elif k=='unset': s.set_flag(v,False)
        elif k=='dialog':
            ev.append(('dialog',v))
            for kk,vv in remake_dialog_effects(v):
                if kk=='set': s.set_flag(vv,True)
                elif kk=='unset': s.set_flag(vv,False)
                elif kk=='hostileattack': ev.append(('hostileattack-d',))
        elif k=='hostileattack':
            if ('hostileattack',) not in ev: ev.append(('hostileattack',))
        elif k in('setfaza','setallfaza'):
            pl=(v.split(None,1)+[''])[:2]; ev.append((k,(pl[0],pl[1].strip())))
        elif k=='startlevel': ev.append((k,v))
        elif k=='tnijitems': ev.append((k,))
        elif k=='zdrowie': ev.append((k,float(v)))
        elif k=='cutscene': ev.append((k,v))
        else: ev.append(('UNSUPPORTED',k))
    def run(s,act,W,ev):
        allowed=True
        for ln,k,v in act['cmds']:
            if k=='endifs': allowed=True
            elif k.startswith('if'):
                if allowed: allowed=s.cond(k,v,W)
            elif allowed: s.execute(k,v,ev)
    def tick(s,dt,W,ev,acts):
        for k in s.timers: s.timers[k]+=dt
        for a in acts: s.run(a,W,ev)
