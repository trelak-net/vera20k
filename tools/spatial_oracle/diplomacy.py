"""Execute original CanAlly501540, ToggleAlliance7318F0 input gates,
and MakeEnemy4F9F90 bit/anger/threat operations up to its UI notification.
Host seams: Side lookup, selected-object owner/id, event construction,
map cursor, threat rebuild and anger sink. No Windows runtime is loaded.
"""
import json
from pathlib import Path
from itertools import product
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX,UC_X86_REG_EAX,UC_X86_REG_EIP
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP,SCENARIO,RULES
from tools.native_oracle import SCRATCH,run_checked,image_sha256
A=SCRATCH+0x10000; STOP=A+0x70000; ARRAY=A+0x71000; COUNTRY=A+0x72000; OBJECT=A+0x73000; VT=A+0x74000; STUB=A+0x75000

def setup(count=3,allies=0,defeated=0,passive=0,civilian=False,mode=True):
 u=machine();u.mem_map(A,0x80000)
 for i in range(count):
  h=A+i*0x10000;t=COUNTRY+i*0x200
  u.mem_write(h+0x30,u32(i));u.mem_write(h+0x34,u32(t));u.mem_write(h+0x1f5,bytes([bool(defeated&(1<<i))]));u.mem_write(h+0x1ec,b"\x01")
  u.mem_write(t+0xbc,u32(3 if i==1 and civilian else 0));u.mem_write(t+0x1a6,bytes([bool(passive&(1<<i))]));u.mem_write(ARRAY+4*i,u32(h))
 u.mem_write(A+0x5788,u32(allies));u.mem_write(0xa8022c,u32(ARRAY));u.mem_write(0xa80238,u32(count));u.mem_write(0xa8b238,u32(mode));u.mem_write(0xa8e7ac,u32(0))
 return u

def invoke(u,pc,obj,args=(),end=STOP):
 u.mem_write(SP,u32(STOP)+b"".join(u32(v) for v in args));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,obj);run_checked(u,pc,end,count=100000)
 return u.reg_read(UC_X86_REG_EAX)&255

def can_ally(count,allies,defeated,passive,civilian,mode,same=False):
 u=setup(count,allies,defeated,passive,civilian,mode)
 def hook(uc,pc,size,data):
  if pc==0x6a46d0:returns(uc,3,0)
 u.hook_add(UC_HOOK_CODE,hook)
 return dict(count=count,allies=allies,defeated=defeated,passive=passive,civilian=civilian,mode=mode,same=same,admitted=bool(invoke(u,0x501540,A,[A if same else A+0x10000])))

def hotkey(selected,dead,same,human,mode,fixed,allowed,change):
 u=setup();u.mem_write(0xa8ecc8,u32(selected));u.mem_write(0xa83d4c,u32(A));u.mem_write(0xa8ecbc,u32(ARRAY+0x100));u.mem_write(ARRAY+0x100,u32(OBJECT));u.mem_write(OBJECT,u32(VT));u.mem_write(VT+0x3c,u32(STUB));u.mem_write(VT+0x38,u32(STUB+4))
 target=A if same else A+0x10000
 u.mem_write(A+0x1f5,bytes([dead]));u.mem_write(target+0x1ec,bytes([human]));u.mem_write(0xa8b238,u32(mode));u.mem_write(SCENARIO,u32(0x400 if fixed else 0));u.mem_write(RULES+0x14b5,bytes([allowed]));u.mem_write(RULES+0x14bb,bytes([change]));admitted=[]
 def hook(uc,pc,size,data):
  if pc==STUB:returns(uc,target,0)
  elif pc==STUB+4:returns(uc,0 if same else 1,0)
  elif pc==0x4c6720:admitted.append(True);uc.reg_write(UC_X86_REG_EIP,STOP)
 u.hook_add(UC_HOOK_CODE,hook);invoke(u,0x7318f0,0)
 return dict(selected=selected,dead=dead,same=same,human=human,mode=mode,fixed=fixed,allowed=allowed,change=change,admitted=bool(admitted))

def break_alliance(ours,theirs):
 u=setup(allies=2 if ours else 0);u.mem_write(A+0x10000+0x5788,u32(1 if theirs else 0));anger=[];threat=[]
 def hook(uc,pc,size,data):
  owner=(uc.reg_read(UC_X86_REG_ECX)-A)//0x10000;sp=uc.reg_read(UC_X86_REG_ESP)
  if pc==0x4adee0:returns(uc,0,8)
  elif pc==0x509400:threat.append(owner);returns(uc,0,0)
  elif pc==0x504790:anger.append([owner,(read32(uc,sp+8)-A)//0x10000,read32(uc,sp+4)]);returns(uc,0,8)
  elif pc==0x4adcd0:returns(uc,0,8)
 u.hook_add(UC_HOOK_CODE,hook)
 # Both input bits present leads through the complete mutation prefix. With
 # ours absent, MakeEnemy jumps directly to its no-change epilogue.
 end=0x4fa0e4 if ours else 0x4fa1da
 invoke(u,0x4f9f90,A,[A+0x10000,1],end=end)
 return dict(ours=ours,theirs=theirs,after=[read32(u,A+0x5788),read32(u,A+0x10000+0x5788)],anger=anger,threat=threat)

def generate():
 cases=[]
 for count,allies,defeated,passive,civilian,mode in product([2,3,5],[0,1,2,5,29],[0,1,2,4],[0,4],[False,True],[False,True]):
  cases.append(can_ally(count,allies,defeated,passive,civilian,mode))
 cases.append(can_ally(3,0,0,0,False,True,True))
 return dict(native_sha256=image_sha256(),coverage="CanAlly live admission; complete hotkey gates; MakeEnemy mutation prefix. Windows dialog, AI paranoid response and multiplayer transport are not executed.",can_ally=cases,hotkey=[hotkey(*flags) for flags in product([False,True],repeat=8)],break_alliance=[break_alliance(a,b) for a,b in product([False,True],repeat=2)])
if __name__=="__main__":
 result=generate();Path(__file__).with_suffix(".json").write_text(json.dumps(result,indent=2)+"\n");print({key:len(value) for key,value in result.items() if isinstance(value,list)})
