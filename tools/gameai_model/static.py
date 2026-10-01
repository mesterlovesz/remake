"""Static same-frame interaction analysis: pairs of actions (script order i<j) in one level whose outcome depends on the order."""
import itertools,collections,sys,json
from sim import *

def info(n):
    """walk the retail chain; collect conditions and the effect node"""
    conds=[];cur=n;eff={}
    while True:
        for k,v in cur.f.items():
            if k=='_lines': continue
            if k in EFFECT: eff[k]=v
            else: conds.append((k,v))
        if cur.next is None: break
        cur=cur.next
    reads=set();lit=set()
    for k,v in conds:
        if k=='if_var': reads.add(v);lit.add((v,True))
        elif k=='ifnot_var': reads.add(v);lit.add((v,False))
        elif k in('iflicznikwiekszyniz',): reads.add(v[1])
        elif k=='ifalive': lit.add(('alive:'+v,True))
        elif k=='ifdead': lit.add(('alive:'+v,False))
        elif k=='ifweapondrawn': lit.add(('drawn',True))
        elif k=='ifweaponhidden': lit.add(('drawn',False))
        elif k=='ifaction': lit.add(('actionwho',v))
    writes={}
    def w(name,val): writes[name]=val
    if 'set' in eff: w(eff['set'],True)
    if 'unset' in eff: w(eff['unset'],False)
    dialogs=[]
    if 'dialog' in eff:
        d=eff['dialog'];dialogs.append(d)
        dd=DIALOG.get(d,{})
        if 'set' in dd: w(dd['set'],True)
        if 'unset' in dd: w(dd['unset'],False)   # unset after set (retail order) - approximate
    return dict(name=n.name,line=n.line,conds=conds,reads=reads,lit=lit,eff=eff,writes=writes,dialogs=dialogs)
def compatible(a,b):
    la=dict();
    for k,v in a['lit']: la.setdefault(k,set()).add(v)
    for k,v in b['lit']:
        if k in la and v not in la[k] and len(la[k])==1: return False
    wa=[v for k,v in a['lit'] if k=='actionwho'];wb=[v for k,v in b['lit'] if k=='actionwho']
    if wa and wb and wa[0]!=wb[0]: return False
    return True
if __name__=='__main__':
    out=collections.defaultdict(list)
    for lv in L:
        acts=[info(n) for n in lv.nodes if n.name]   # script order
        for i,j in itertools.combinations(range(len(acts)),2):
            a,b=acts[i],acts[j]
            if not compatible(a,b): continue
            kinds=[]
            # (1) both start a dialog: last one wins
            if a['dialogs'] and b['dialogs'] and a['dialogs']!=b['dialogs']: kinds.append('both-dialog')
            for k in ('startlevel','cutscene'):
                if k in a['eff'] and k in b['eff']: kinds.append('both-'+k)
            # (2) data dependency through a flag
            for v,val in a['writes'].items():
                if v in b['reads']: kinds.append(f'{a["name"]}@{a["line"]} writes {v} read by {b["name"]}@{b["line"]}')
            for v,val in b['writes'].items():
                if v in a['reads']: kinds.append(f'{b["name"]}@{b["line"]} writes {v} read by {a["name"]}@{a["line"]}')
            # (3) same flag written by both with different values
            for v in set(a['writes'])&set(b['writes']):
                if a['writes'][v]!=b['writes'][v]: kinds.append(f'write-write {v}')
            # (4) setfaza to same target
            sa=a['eff'].get('setfaza');sb=b['eff'].get('setfaza')
            if sa and sb and sa[1]==sb[1] and sa[0]!=sb[0]: kinds.append('setfaza-same-target')
            if kinds: out[lv.name].append((a['name'],a['line'],b['name'],b['line'],kinds))
    tot=0
    for lv,rows in out.items():
        print(lv,len(rows));tot+=len(rows)
        if '-v' in sys.argv:
            for r in rows: print('   ',r)
    print(tot)
