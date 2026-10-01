"""Lists every EffectMgr::Spawn call of cshell.dll (0x1004e520) with its decoded arguments (docs/retail-blast.md).

Walks back from each `call 0x1004e520` over the 19 `push` instructions (arg1 is pushed last). Immediates are printed as numbers, floats or the
string they point at; registers stay symbolic (nested calls between the pushes can confuse the walk: check odd rows by hand).
Usage: python -m tools.spawn_sites [address prefix ...]"""
import re, struct, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'research/python-deps'))
import pefile, capstone

NAMES = {1: 'type', 2: 'life', 3: 'moves', 4: 'path', 5: 'pos', 6: 'vel', 7: 'grav', 8: 'spin', 9: 'scale', 10: 'color', 12: 'a12', 13: 'mult', 14: 'a14',
         15: 'a15', 16: 'normal', 17: 'fade', 18: 'growth', 19: 'quat'}

def main():
    pe = pefile.PE(str(Path(__file__).resolve().parents[2] / 'GYARI/cshell.dll')); base = pe.OPTIONAL_HEADER.ImageBase
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    lines = []
    for s in pe.sections:
        if s.Characteristics & 0x20000000:
            code = s.get_data(); addr = base + s.VirtualAddress; off = 0
            while off < len(code):
                got = False
                for ins in md.disasm(code[off:], addr + off):
                    got = True; off = ins.address + ins.size - addr; lines.append((ins.address, f'{ins.mnemonic} {ins.op_str}'))
                if not got: off += 1
    def text(a):
        try:
            d = pe.get_data(a - base, 80).split(b'\0')[0]
        except Exception: return None
        if not d: return '""'
        return d.decode() if len(d) >= 3 and all(32 <= c < 127 for c in d) else None
    def value(op):
        m = re.match(r'push (0x[0-9a-f]+|\d+)$', op)
        if not m: return op[5:]
        v = int(m[1], 0) & 0xffffffff
        s = text(v) if 0x10000000 <= v < 0x10100000 else None
        if s is not None: return s
        f = struct.unpack('<f', struct.pack('<I', v))[0]
        return str(v) if v <= 64 else ('%g' % f if 1e-3 < abs(f) < 1e6 else hex(v))
    wanted = sys.argv[1:]
    for i, (a, t) in enumerate(lines):
        if t != 'call 0x1004e520' or (wanted and not any(f'{a:x}'.startswith(w) for w in wanted)): continue
        pushes = []; j = i - 1
        while j >= 0 and len(pushes) < 19 and i - j < 140:
            if lines[j][1].startswith('push '): pushes.append(value(lines[j][1]))
            j -= 1
        print(f'{a:08x}', ' '.join(f'{NAMES[n]}={pushes[n - 1] if n - 1 < len(pushes) else "?"}' for n in NAMES))

if __name__ == '__main__':
    main()
