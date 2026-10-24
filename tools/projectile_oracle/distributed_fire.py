"""Original709550; only allocation/free and AssignTarget are observed host seams."""
import json
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP
from tools.native_oracle import SCRATCH,run_checked,image_sha256
ACTOR=SCRATCH+0x10000;VT=SCRATCH+0x11000;CANDIDATES=SCRATCH+0x12000
SCORES=SCRATCH+0x13000;HISTORY=SCRATCH+0x14000;SINK=SCRATCH+0x15000;STOP=SINK+16

def run(case):
 u=machine();u.mem_map(SCRATCH+0x10000,0x70000)
 u.mem_write(ACTOR,u32(VT));u.mem_write(VT+0x3c8,u32(SINK))
 u.mem_write(ACTOR+0x45c,u32(CANDIDATES));u.mem_write(ACTOR+0x468,u32(len(case['candidates'])))
 u.mem_write(ACTOR+0x444,u32(SCORES))
 for i,(target,score) in enumerate(case['candidates']):u.mem_write(CANDIDATES+4*i,u32(target));u.mem_write(SCORES+4*i,u32(score))
 for off,val in [(0x470,0x7e91ec),(0x474,HISTORY),(0x478,64),(0x47c,0x101),(0x480,len(case['history'])),(0x484,10)]:u.mem_write(ACTOR+off,u32(val))
 for i,target in enumerate(case['history']):u.mem_write(HISTORY+4*i,u32(target))
 heap=SCRATCH+0x20000;targets=[]
 def hook(uc,pc,size,data):
  nonlocal heap
  sp=uc.reg_read(UC_X86_REG_ESP)
  if pc==0x7c8e17:
   n=read32(uc,sp+4);assert 0<n<0x10000
   ptr=heap;heap+=(n+15)&~15;returns(uc,ptr,0)
  elif pc==0x7c8b3d:returns(uc,0,0)
  elif pc==SINK:targets.append(read32(uc,sp+4));returns(uc,0,4)
 u.hook_add(UC_HOOK_CODE,hook)
 u.mem_write(SP,u32(STOP));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,ACTOR)
 run_checked(u,0x709550,STOP,count=100000)
 n=read32(u,ACTOR+0x480);ptr=read32(u,ACTOR+0x474)&0xffffffff
 return dict(input=case,assigned=targets,history=[read32(u,ptr+4*i) for i in range(n)])
def generate():
 cases=[]
 for candidates in [[],[(1,5)],[(1,5),(2,8),(3,8)],[(1,8),(2,8)],[(1,0),(2,-1)]]:
  for history in [[],[1],[2],[1,2,3],[9],[1,1,2]]:cases.append(dict(candidates=candidates,history=history))
 return dict(native_sha256=image_sha256(),rows=[run(c) for c in cases])
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print('Generated',len(data['rows']),'original cases')
