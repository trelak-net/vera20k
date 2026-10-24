"""Execute SecretLab selection68C050 and GetSecretProduction459840.
Only allocator/free and supplied Scenario RNG results are host seams.
"""
import json,itertools
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX,UC_X86_REG_EAX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP,RULES
from tools.native_oracle import SCRATCH,run_checked,image_sha256
LABS=SCRATCH+0x10000;TYPES=SCRATCH+0x20000;ARRAY=SCRATCH+0x30000;STOP=ARRAY+0xff0

def selection(counts,labs,policy):
 u=machine();u.mem_map(SCRATCH+0x10000,0x70000)
 n=sum(counts);pool=[]
 for family,(off,count) in enumerate(zip([0xd00,0xd1c,0xd38],counts)):
  ptr=ARRAY+0x100+family*0x100;u.mem_write(RULES+off+4,u32(ptr));u.mem_write(RULES+off+16,u32(count))
  for i in range(count):
   value=100*family+i+1;pool.append(value);u.mem_write(ptr+4*i,u32(value))
 u.mem_write(RULES+0xd54,u32(n));u.mem_write(0x8b41e4,u32(ARRAY));u.mem_write(0x8b41f0,u32(labs))
 for i in range(labs):u.mem_write(ARRAY+4*i,u32(LABS+i*0x800))
 calls=[];heap=SCRATCH+0x40000
 def hook(uc,pc,size,data):
  nonlocal heap
  sp=uc.reg_read(UC_X86_REG_ESP)
  if pc==0x7c8e17:
   size=read32(uc,sp+4);assert 0<size<0x10000
   ptr=heap;heap+=(size+15)&~15;returns(uc,ptr,0)
  elif pc==0x7c8b3d:returns(uc,0,0)
  elif pc==0x65c7e0:
   lo,hi=read32(uc,sp+4),read32(uc,sp+8)
   draw={'first':lo,'last':hi,'middle':(lo+hi)//2}[policy]
   calls.append([lo,hi,draw]);returns(uc,draw,8)
 u.hook_add(UC_HOOK_CODE,hook);u.mem_write(SP,u32(STOP));u.reg_write(UC_X86_REG_ESP,SP)
 run_checked(u,0x68c050,STOP,count=100000)
 return dict(counts=counts,labs=labs,policy=policy,pool=pool,calls=calls,rewards=[read32(u,LABS+i*0x800+0x6f4) for i in range(labs)])
def getter(values):
 u=machine();u.mem_map(SCRATCH+0x10000,0x30000)
 u.mem_write(LABS+0x520,u32(TYPES))
 for off,val in zip([0xea4,0xea8,0xeac],values):u.mem_write(TYPES+off,u32(val))
 u.mem_write(LABS+0x6f4,u32(values[3]));u.mem_write(SP,u32(STOP));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,LABS)
 run_checked(u,0x459840,STOP,count=1000)
 return dict(values=values,reward=u.reg_read(UC_X86_REG_EAX))
def generate():
 return dict(native_sha256=image_sha256(),selection=[selection(c,n,p) for c in [[0,0,0],[2,0,0],[0,3,0],[0,0,2],[2,3,2]] for n in [0,1,2,3,8] for p in ['first','last','middle']],getter=[getter([v*(i+1) for i,v in enumerate(bits)]) for bits in itertools.product([0,1],repeat=4)])
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print('Executed',len(data['selection']),'selection cases and',len(data['getter']),'getter cases');print([r for r in data['selection'] if r['counts']==[2,3,2] and r['labs']==3])
