import itertools,collections
from static import *
rows=[]
for lv in L:
    acts=[info(n) for n in lv.nodes if n.name]
    for i,j in itertools.combinations(range(len(acts)),2):
        a,b=acts[i],acts[j]
        if not compatible(a,b): continue
        for (x,y,who) in ((a,b,'earlier writes, later suppressed in remake'),(b,a,'later writes, earlier suppressed in retail')):
            for v,val in x['writes'].items():
                # y requires the opposite of what x writes
                need=(v,not val)
                if need in y['lit'] and (v,val) not in y['lit']:
                    rows.append((lv.name,a['name'],a['line'],b['name'],b['line'],v,val,who,b['dialogs'] or a['dialogs']))
print(len(rows))
seen=set()
for r in rows:
    print(r)
