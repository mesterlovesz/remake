"""Read-only x86 evidence helper. Addresses are virtual addresses of the chosen retail module (default cshell.dll)."""
from pathlib import Path
import sys, struct, argparse
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'research/python-deps'))
import pefile, capstone

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('start',type=lambda s:int(s,16))
    ap.add_argument('end',type=lambda s:int(s,16))
    ap.add_argument('--module',default='cshell.dll',help='retail module in GYARI (cshell.dll, object.lto, Lithtech.exe)')
    args=ap.parse_args()
    pe=pefile.PE(str(Path(__file__).resolve().parents[2]/'GYARI'/args.module))
    base=pe.OPTIONAL_HEADER.ImageBase
    md=capstone.Cs(capstone.CS_ARCH_X86,capstone.CS_MODE_32)
    import re
    code=pe.get_data(args.start-base,args.end-args.start)
    def sweep():
        # Linear sweep that resynchronises after undecodable bytes instead of stopping.
        offset=0
        while offset<len(code):
            decoded=False
            for ins in md.disasm(code[offset:],args.start+offset):
                decoded=True; offset=ins.address+ins.size-args.start; yield ins
            if not decoded or offset<len(code) and not any(True for _ in md.disasm(code[offset:offset+15],0)):
                offset+=1
    for ins in sweep():
        note=''
        match=re.search(r'\[(0x[0-9a-f]+)\]',ins.op_str)
        if match and ins.mnemonic.startswith('f'):
            address=int(match[1],16)
            try: note=f' ; float={struct.unpack("<f",pe.get_data(address-base,4))[0]}'
            except Exception: pass
        # Immediates that are plausible IEEE floats (|value| 1e-3..1e6), e.g. `mov dword ptr [esi + 0x130], 0x42680000`.
        imm=re.search(r', (0x[0-9a-f]{8})$',ins.op_str)
        if imm and ins.mnemonic in ('mov','push','cmp'):
            value=struct.unpack('<f',struct.pack('<I',int(imm[1],16)))[0]
            if 1e-3<abs(value)<1e6: note=f' ; float={value:g}'
        print(f'{ins.address:08x} {ins.mnemonic:8s} {ins.op_str}{note}')
if __name__=='__main__': main()
