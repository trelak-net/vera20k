"""Execute ChangeHouse6E0AA0 and Grant6CB560 from the installed original.
The action's house lookup, Tag.Contains and SetOwner are supplied host seams;
Grant executes its actual timer and readiness code, with audio suppressed.
"""
import json,itertools
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX,UC_X86_REG_EAX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP
from tools.native_oracle import SCRATCH,run_checked,image_sha256
A=SCRATCH+0x10000;B=A+0x1000;T=A+0x2000;V=A+0x4000;STOP=A+0xf000;SET=STOP+16

def change(value,live,resolve):
 u=machine();u.mem_map(A,0x10000);u.mem_write(A+0x90,u32(value));u.mem_write(0xa8ec7c,u32(B));u.mem_write(0xa8ec88,u32(16));u.mem_write(V+0x3d4,u32(SET))
 rows=[]
 for i,bits in enumerate(itertools.product([0,1],repeat=4)):
  alive,marked,limbo,attached=bits;ptr=T+i*0x100
  u.mem_write(B+4*i,u32(ptr));u.mem_write(ptr,u32(V));u.mem_write(ptr+0x90,bytes([alive]));u.mem_write(ptr+0x74,bytes([marked]));u.mem_write(ptr+0x81,bytes([limbo]));u.mem_write(ptr+0x34,u32(A+0x8000+i*4 if attached else 0));rows.append(list(bits))
 calls=[];lookups=[]
 def hook(uc,pc,size,data):
  sp=uc.reg_read(UC_X86_REG_ESP);ecx=uc.reg_read(UC_X86_REG_ECX)
  if pc==0x510f60:return returns(uc,int(4475<=value<=4482),0)
  if pc in [0x510ed0,0x502d30,0x726910]:
   lookups.append(hex(pc));return returns(uc,0x123456 if resolve else 0,0)
  if pc==0x6e5380:return returns(uc,int(read32(uc,sp+4)==live),4)
  if pc==SET:
   calls.append([(ecx-T)//0x100,read32(uc,sp+4),read32(uc,sp+8)]);return returns(uc,1,8)
 u.hook_add(UC_HOOK_CODE,hook);u.mem_write(SP,b''.join(u32(v) for v in [STOP,0,0,live,0]));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,A)
 run_checked(u,0x6e0aa0,STOP,count=10000)
 return dict(value=value,live=live,resolve=resolve,rows=rows,calls=calls,lookups=lookups,result=u.reg_read(UC_X86_REG_EAX)&255)
def grant(once,active,ready,manual):
 u=machine();u.mem_map(A,0x10000);u.mem_write(A+0x24,u32(-1));u.mem_write(A+0x28,u32(T));u.mem_write(A+0x2c,u32(B));u.mem_write(A+0x30,u32(11));u.mem_write(A+0x38,u32(123));u.mem_write(A+0x60,b'\1');u.mem_write(A+0x6d,bytes([active,0,ready,0]));u.mem_write(T+0xb0,u32(4500));u.mem_write(T+0xb4,u32(11));u.mem_write(T+0xf5,bytes([manual]));u.mem_write(0xa8ed84,u32(100))
 def hook(uc,pc,size,data):
  if pc==0x752700:returns(uc,0,4)
 u.hook_add(UC_HOOK_CODE,hook);u.mem_write(SP,b''.join(u32(v) for v in [STOP,once,0,0]));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,A)
 run_checked(u,0x6cb560,STOP,count=10000)
 return dict(once=once,active=active,ready=ready,manual=manual,flags=list(u.mem_read(A+0x6d,4)),timer=[read32(u,A+0x30),read32(u,A+0x38)])
def generate():return dict(native_sha256=image_sha256(),change=[change(v,l,r) for v in [-1,0,4475,4482,8997] for l in [0,77] for r in [False,True]],grant=[grant(*v) for v in itertools.product([0,1],repeat=4)])
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix(".json").write_text(json.dumps(data,indent=2)+'\n');print('Executed',len(data['change']),'ChangeHouse cases and',len(data['grant']),'Grant cases');print(data['grant'])
