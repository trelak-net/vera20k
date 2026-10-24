"""Execute native geometry761640 through endpoint stores761B75, then UpdateCells7610F0.
Only SetLocation, vector operations and map-cell lookup are host seams. The
geometry prefix proves B4=firer, C0=target+50Z: older Rust comments reversed them.
No Windows/device/audio code runs. Cell coordinates here are within a flat map.
"""
import json,struct
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP
from tools.native_oracle import SCRATCH,run_checked,image_sha256
A=SCRATCH+0x10000;WAVE=A;VT=A+0x1000;SRC=A+0x2000;DST=SRC+16;STOP=A+0x3000;STUB=STOP+16

def native(source,target,active,octant,fade,fade_out=0.0):
 u=machine();u.mem_map(A,0x20000);u.mem_write(WAVE,u32(VT));u.mem_write(VT+0x1b4,u32(STUB));u.mem_write(SRC,struct.pack('<iii',*source));u.mem_write(DST,struct.pack('<iii',*target))
 # Skip one-time static mesh initialization; this prefix never reads that mesh.
 u.mem_write(0xb45da0,b"\x01")
 lookups={};reverse={}
 vector=A+0x18000
 u.mem_write(WAVE+0x1f4,u32(vector));u.mem_write(WAVE+0x1f8,u32(128))
 def hook(uc,pc,size,data):
  sp=uc.reg_read(UC_X86_REG_ESP)
  if pc==STUB:return returns(uc,0,4)
  if pc==STUB+16:uc.mem_write(WAVE+0x200,u32(0));return returns(uc,0,0)
  if pc==0x5657a0:
   cell=struct.unpack('<hh',uc.mem_read(read32(uc,sp+4),4))
   ptr=lookups.setdefault(cell,A+0x4000+len(lookups)*0x200);reverse[ptr]=cell
   return returns(uc,ptr,4)
  if pc==STUB+32:
   ptr=read32(uc,read32(uc,sp+4));items=[read32(uc,vector+4*i) for i in range(read32(uc,WAVE+0x200))]
   return returns(uc,items.index(ptr) if ptr in items else -1,4)
  if pc==0x763330:
   count=read32(uc,WAVE+0x200);uc.mem_write(vector+4*count,u32(read32(uc,read32(uc,sp+4))));uc.mem_write(WAVE+0x200,u32(count+1));return returns(uc,1,4)
 u.hook_add(UC_HOOK_CODE,hook)
 u.mem_write(SP,u32(STOP)+u32(SRC)+u32(DST));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,WAVE)
 run_checked(u,0x761640,0x761b78,count=100000)
 endpoints=[list(struct.unpack('<iii',u.mem_read(WAVE+off,12))) for off in [0xb4,0xc0]]
 assert endpoints==[source,[target[0],target[1],target[2]+50]],endpoints
 # Known edge coordinates, independent of the geometry helper's later work.
 edges=[[source[0],source[1]-100,source[2]],[source[0],source[1]+100,source[2]],
        [target[0],target[1]-100,target[2]+50],[target[0],target[1]+100,target[2]+50]]
 for off,edge in zip([0xfc,0x108,0x114,0x120],edges):u.mem_write(WAVE+off,struct.pack('<iii',*edge))
 u.mem_write(WAVE+0x1f0,u32(VT));u.mem_write(VT+0xc,u32(STUB+16));u.mem_write(VT+0x10,u32(STUB+32))
 u.mem_write(WAVE+0x12c,bytes([active]));u.mem_write(WAVE+0x1cc,u32(octant));u.mem_write(WAVE+0x138,struct.pack('<d',fade));u.mem_write(WAVE+0x140,struct.pack('<d',fade_out))
 u.mem_write(SP,u32(STOP));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,WAVE)
 run_checked(u,0x7610f0,STOP,count=100000)
 cells=[reverse[read32(u,vector+4*i)] for i in range(read32(u,WAVE+0x200))]
 return dict(source=source,target=target,active=active,octant=octant,fade=fade,fade_out=fade_out,edges=edges,endpoints=endpoints,cells=[list(c) for c in cells])
def generate():
 rows=[]
 for source,target in [([3456,3200,0],[2944,3200,0]),([1024,1024,0],[255,255,104]),([128,128,0],[1152,128,0])]:
  for active in [True,False]:
   for octant in [3,4]:
    for fade in [struct.unpack('<f',struct.pack('<f',.05))[0],1.0]:rows.append(native(source,target,active,octant,fade))
 step=struct.unpack('<f',struct.pack('<f',.05))[0]
 for octant in [3,4]: rows.append(native([1024,1024,0],[255,255,0],False,octant,step,step))
 negative=native([-1024,-1024,0],[-256,-256,0],True,3,1.0)
 return dict(native_sha256=image_sha256(),rows=rows,negative=negative)
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print('Executed',len(data['rows']),'original sonic endpoint/cell controls')
