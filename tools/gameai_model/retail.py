"""Model of cshell.dll gameai.txt parser (0x10015cc0) and executor (0x1001a140)."""
import re
import pathlib
ROOT=pathlib.Path(__file__).resolve().parents[2]
SCRIPT=str(ROOT/'output'/'decoded_scripts'/'gameai.txt')
COND={ # keyword -> (field, arg kind)   (parser address)
 'if':'if_var','ifnot':'ifnot_var','ifactionhostile':'ifactionhostile','ifseenbyhostile':'ifseenbyhostile',
 'ifplayerseenby':'ifplayerseenby','ifplayerhas':'ifplayerhas','ifweapondrawn':'ifweapondrawn','ifweaponhidden':'ifweaponhidden',
 'ifalive':'ifalive','ifdead':'ifdead','ifaction':'ifaction','iflicznikwiekszyniz':'iflicznikwiekszyniz',
 'iflicznikmniejszyniz':'iflicznikmniejszyniz','ifgraczblizejniz':'ifgraczblizejniz','ifgraczdalejniz':'ifgraczdalejniz',
 'ifhostileblizejniz':'ifhostileblizejniz'}
EFFECT=['dialog','startlevel','cutscene','hostileattack','tnijitems','set','unset','setfaza','setallfaza','wlaczmuze','print','zdrowie','outro']
# executor order of effects inside one node (0x1001a4ad..0x1001a71f)
EXEC_ORDER=['dialog','startlevel','cutscene','hostileattack','tnijitems','set','unset','setfaza','setallfaza','print','zdrowie','outro']
LEVELFIELDS=['muza','load_c','load_i','load_o','load_w','load_d','gestosc_sciezek','nie_sprawdzaj_drzwi','pure_shooter','include','bool','licznik','level','action']

def first_word(line):   # 0x100581a0: up to first ' ' or NUL, max 127
    w=''
    for ch in line[:127]:
        if ch==' ': break
        w+=ch
    return w
def rest(line):         # 0x10058230: drop leading spaces, trailing spaces, first word, following spaces
    s=line.lstrip(' ').rstrip(' ')
    i=s.find(' ')
    if i<0: return ''
    return s[i:].lstrip(' ')
def atof(s):
    m=re.match(r'\s*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?',s)
    return float(m.group(0)) if m else 0.0

class Node:
    __slots__=('name','f','next','line')
    def __init__(s,line=0): s.name='';s.f={};s.next=None;s.line=line
class Level:
    def __init__(s,name): s.name=name;s.nodes=[];s.line=0;s.pure=False
def parse(text):
    """returns (levels, vars_)  vars_: name->dict(kind,...) global list (newest first lookup)"""
    levels=[];vars_=[];S=None
    cur=None;log=[]
    lines=text.replace('\r\n','\n').split('\n')
    for ln,line in enumerate(lines,1):
        w=first_word(line); r=rest(line)
        if w=='bool': vars_.append(dict(name=r,bool=True,ctr=False,flag=False,val=0.0,line=ln))
        if w=='licznik': vars_.append(dict(name=r,bool=False,ctr=True,flag=False,val=0.0,line=ln))
        if w=='level':
            cur=Level(r);cur.line=ln;levels.append(cur);S=None
        if w=='pure_shooter' and cur: cur.pure=True
        if w=='action':
            n=Node(ln);n.name=r;cur.nodes.append(n);S=None
            continue
        if w in COND or w in EFFECT:
            if cur is None or not cur.nodes: log.append((ln,'NODELESS',line));continue
            if w in COND:
                if S is not None:
                    n=Node(ln);cur.nodes.append(n);S.next=n
                head=cur.nodes[-1];S=head
                fld=COND[w]
                if fld in('iflicznikwiekszyniz','iflicznikmniejszyniz','ifgraczblizejniz','ifgraczdalejniz'):
                    head.f[fld]=(atof(r),rest(r))
                elif fld=='ifhostileblizejniz': head.f[fld]=atof(r)
                elif fld in('ifactionhostile','ifseenbyhostile','ifweapondrawn','ifweaponhidden'): head.f[fld]=True
                else: head.f[fld]=r
                head.f.setdefault('_lines',[]).append(ln)
            else:
                if S is not None:
                    n=Node(ln);cur.nodes.append(n);S.next=n
                S=None
                head=cur.nodes[-1]
                if w in('hostileattack','tnijitems','outro'): v=True
                elif w=='zdrowie': v=atof(r)
                elif w in('setfaza','setallfaza'):
                    ph=r; nm=rest(r); ph=ph.split(' ')[0] if ph else ''   # phase cut at first space (0x1001700d loop)
                    v=(ph,nm)
                else: v=r
                if w in head.f and w not in COND: log.append((ln,'OVERWRITE',w,head.f[w],v))
                head.f[w]=v
                head.f.setdefault('_lines',[]).append(ln)
    return levels,vars_,log

def find_var(vars_,name):   # 0x10015ac0, newest first
    for v in reversed(vars_):
        if v['name']==name: return v
    return None

def actions_of(level):   # executor iterates the level list newest first, only nodes with a name
    return [n for n in reversed(level.nodes) if n.name!='']
if __name__=='__main__':
    t=open(SCRIPT,encoding='cp1250').read()
    L,V,log=parse(t)
    print(len(L),'levels',sum(len(actions_of(l)) for l in L),'actions',len(V),'vars')
    for l in log: print(l)
