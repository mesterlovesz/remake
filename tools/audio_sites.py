"""Enumerates every PlaySound call site of the retail game modules (read-only evidence tool for docs/retail-audio-parity.md).

The engine's sound manager is reached two ways: `ILTClient->GetSoundMgr()` (vtable +0x14) followed by `PlaySound(PlaySoundInfo*, handle*)`
(vtable +4), or the cached manager pointer with `PlaySound` at +4. Every site builds a PlaySoundInfo on the stack: flags first (2 = 3D at a
position, 0x200 = local/2D, 0x4 loop, 0x10 keep the handle, 0x1000 / 0x400 speech / effect class), the file name at +4, radius and volume near +0x118.
The game wraps them in a handful of small functions; this tool finds the wrappers and lists their callers with the file names pushed right before.

  python -m tools.audio_sites                    # cshell.dll and object.lto
  python -m tools.audio_sites --module cshell.dll --context 14
"""
import argparse, re, struct, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'research/python-deps'))
import pefile, capstone

GYARI = Path(__file__).resolve().parents[2] / 'GYARI'


def disassemble(module):
    pe = pefile.PE(str(GYARI / module))
    base = pe.OPTIONAL_HEADER.ImageBase
    section = next(s for s in pe.sections if s.Name.startswith(b'.text'))
    code, start = section.get_data(), base + section.VirtualAddress
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    rows, offset = [], 0
    while offset < len(code):
        decoded = False
        for ins in md.disasm(code[offset:], start + offset):
            decoded = True
            offset = ins.address + ins.size - start
            rows.append((ins.address, ins.mnemonic, ins.op_str))
        if not decoded or (offset < len(code) and not list(md.disasm(code[offset:offset + 15], 0))):
            offset += 1
    return pe, base, rows


def text_at(pe, base, address):
    try:
        raw = pe.get_data(address - base, 96).split(b'\0')[0]
        if len(raw) >= 2 and all(32 <= c < 127 for c in raw):
            return raw.decode()
    except Exception:
        pass
    return None


def play_sites(rows):
    """Addresses of `call [reg+0x14]` ... `call [reg+4]` (GetSoundMgr then PlaySound) and of `mov ecx,[mgr]` ... `call [reg+4]` with a flags store."""
    sites = []
    for i, (address, mnemonic, ops) in enumerate(rows):
        if mnemonic == 'call' and re.fullmatch(r'dword ptr \[e.x \+ 0x14\]', ops):
            if any(m == 'call' and re.fullmatch(r'dword ptr \[e.x \+ 4\]', o) for _, m, o in rows[i + 1:i + 8]):
                sites.append(address)
    return sites


PLAY_FLAGS = {0x2, 0x200, 0x254, 0x412, 0x416, 0x1012, 0x1210}


def cached_sites(rows):
    """`call [reg+4]` through a cached manager pointer (`mov ecx,[global]`), recognised by a PlaySoundInfo flags store shortly before."""
    sites = []
    for i, (address, mnemonic, ops) in enumerate(rows):
        if mnemonic == 'call' and re.fullmatch(r'dword ptr \[e.x \+ 4\]', ops):
            window = rows[max(0, i - 60):i]
            flags = [int(m[1], 16) for _, mm, o in window if mm == 'mov' and (m := re.search(r'dword ptr \[esp \+ 0x[0-9a-f]+\], (0x[0-9a-f]+)$', o)) and int(m[1], 16) in PLAY_FLAGS]
            if flags and any(mm == 'mov' and re.fullmatch(r'ecx, dword ptr \[0x[0-9a-f]+\]', o) for _, mm, o in rows[max(0, i - 12):i]):
                sites.append((address, flags[-1]))
    return sites


def containing_function(rows, index):
    """Start of the function containing row `index`: the first instruction after the previous `ret` + padding."""
    j = index
    while j > 0:
        if rows[j - 1][1] in ('ret', 'int3') or (rows[j - 1][1] == 'nop' and rows[j][1] != 'nop'):
            return rows[j][0]
        j -= 1
    return rows[0][0]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--module', action='append', help='cshell.dll / object.lto (default both)')
    parser.add_argument('--context', type=int, default=12, help='instructions shown before each caller')
    args = parser.parse_args()
    for module in args.module or ['cshell.dll', 'object.lto']:
        pe, base, rows = disassemble(module)
        index = {address: i for i, (address, _, _) in enumerate(rows)}
        sites = play_sites(rows)
        # A wrapper is the nearest call target before the site (the padding heuristic splits wrappers that contain an early `ret`).
        targets = sorted({int(o, 16) for _, m, o in rows if m == 'call' and re.fullmatch(r'0x[0-9a-f]+', o)})
        def wrapper_of(address):
            near = [t for t in targets if t <= address and address - t < 0x300]
            return max(near) if near else containing_function(rows, index[address])
        wrappers = sorted({wrapper_of(a) for a in sites})
        print(f'== {module}: {len(sites)} direct PlaySound sites in {len(wrappers)} functions: ' + ', '.join(f'0x{w:x}' for w in wrappers))
        for address, flags in cached_sites(rows):
            print(f'-- cached-manager PlaySound at 0x{address:x} (function 0x{containing_function(rows, index[address]):x}), flags 0x{flags:x}')
        for wrapper in wrappers:
            callers = [a for a, m, o in rows if m == 'call' and o == f'0x{wrapper:x}']
            print(f'-- wrapper 0x{wrapper:x}: {len(callers)} callers')
            for caller in callers:
                i = index[caller]
                notes = []
                for a, m, o in rows[max(0, i - args.context):i]:
                    hit = re.search(r'0x(1[0-9a-f]{7}|[4-5][0-9a-f]{5})\b', o)
                    text = text_at(pe, base, int(hit[1], 16)) if hit else None
                    if text:
                        notes.append(f'"{text}"')
                    imm = re.search(r', (0x[0-9a-f]{8})$', o)
                    if imm and m in ('push', 'mov'):
                        value = struct.unpack('<f', struct.pack('<I', int(imm[1], 16)))[0]
                        if 64 <= abs(value) <= 1e5 and value == int(value):
                            notes.append(f'radius/float {value:g}')
                print(f'   0x{caller:x}  function 0x{containing_function(rows, i):x}  ' + ' '.join(notes))


if __name__ == '__main__':
    main()
