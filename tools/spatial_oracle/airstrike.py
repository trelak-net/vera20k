"""Original Airstrike admission41D7E0 and target tint70E920/70E4B0.
Only IsElite and RTTI predicates are supplied; timers, RNG and formulas run.
"""
import json
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX,UC_X86_REG_EAX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP,SCENARIO
from tools.native_oracle import SCRATCH,run_checked,image_sha256
A=SCRATCH+0x10000;MAN=A;OBJECT=A+0x1000;VT=A+0x2000;STUB=A+0x3000;STOP=A+0x4000

def invoke(u,pc,obj,args=()):
 u.mem_write(SP,u32(STOP)+b''.join(u32(a) for a in args));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,obj);run_checked(u,pc,STOP,count=100000)
 return read32(u,SP) if False else u.reg_read(UC_X86_REG_EAX)

def admission(active,same,elite,frame):
 u=machine();u.mem_map(A,0x10000);u.mem_write(MAN+0x4c,u32(OBJECT));u.mem_write(MAN+0x50,u32(123 if same else 0));u.mem_write(MAN+0x3c,bytes([active]));u.mem_write(MAN+0x40,u32(50)+u32(100)+u32(50));u.mem_write(0xa8ed84,u32(frame))
 def hook(uc,pc,size,data):
  if pc==0x750010:returns(uc,elite,0)
 u.hook_add(UC_HOOK_CODE,hook);result=invoke(u,0x41d7e0,MAN,[123])&255
 return dict(active=active,same=same,elite=elite,frame=frame,admitted=bool(result))

def tint(stage,left,lifetime):
 u=machine();u.mem_map(A,0x10000);u.mem_write(OBJECT,u32(VT));u.mem_write(VT+0x2c,u32(STUB));u.mem_write(OBJECT+0x294,u32(MAN));u.mem_write(MAN+0x50,u32(OBJECT));u.mem_write(0xa8ed84,u32(1000))
 invoke(u,0x65c6d0,SCENARIO+0x218,[7]);u.mem_write(OBJECT+0x1a8,u32(1000)+u32(0)+u32(lifetime));u.mem_write(OBJECT+0x1b4,u32(1000)+u32(0)+u32(left));u.mem_write(OBJECT+0x1c0,u32(stage));draws=[]
 def hook(uc,pc,size,data):
  if pc==STUB:returns(uc,6,0)
  if pc==0x65c7e0:draws.append([read32(uc,uc.reg_read(UC_X86_REG_ESP)+4),read32(uc,uc.reg_read(UC_X86_REG_ESP)+8)])
 u.hook_add(UC_HOOK_CODE,hook);invoke(u,0x70e920,OBJECT)
 new=read32(u,OBJECT+0x1c0);duration=read32(u,OBJECT+0x1bc);value=invoke(u,0x70e4b0,OBJECT,[1000]);value=value-(1<<32) if value&(1<<31) else value
 return dict(stage=stage,left=left,lifetime=lifetime,new_stage=new,duration=duration,intensity=value,draws=draws)
def generate():
 return dict(native_sha256=image_sha256(),admission=[admission(a,s,e,f) for a in [False,True] for s in [False,True] for e in [False,True] for f in [49,50,99,100,101,149,150,151]],tint=[tint(s,l,t) for s in range(11) for l in [0,1,64] for t in [29,30,157,158,100000]])
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print({k:len(v) for k,v in data.items() if isinstance(v,list)})
