"""Original Magnetron admission4694CB, release70FEE0 and held-fall54CA90.
Virtual predicates and allocations are supplied/observed at explicit host seams.
The actual instructions decide admission, release order, state writes and x87 fall.
No Windows services, graphics or sound code execute.
"""
import json,struct
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX,UC_X86_REG_EAX,UC_X86_REG_ESI,UC_X86_REG_EDI
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP
from tools.native_oracle import SCRATCH,run_checked,image_sha256
A=SCRATCH+0x10000;SOURCE=A;TARGET=A+0x2000;WH=A+0x4000;BULLET=A+0x5000;VT=A+0x6000;TYPE=A+0x7000;STUB=A+0x9000;CELL=A+0xa000;BUILDING=A+0xb000;STOP=A+0xc000

def admission(case):
 u=machine();u.mem_map(A,0x20000)
 u.mem_write(SOURCE+0x14,b'\4');u.mem_write(SOURCE+0x2ac,u32(TARGET if case['same'] else (TARGET+0x1000 if case['previous'] else 0)));u.mem_write(SOURCE+0x2b0,u32(TARGET if case['source_held'] else 0));u.mem_write(SOURCE+0x6ad,bytes([case['source_swapped']]))
 u.mem_write(TARGET,u32(VT));u.mem_write(TARGET+0x14,bytes([4 if case['foot'] else 0]));u.mem_write(TARGET+0x6ad,bytes([case['swapped']]))
 for off,stub in [(0x2c,STUB),(0x78,STUB+16),(0x160,STUB+32),(0x84,STUB+48)]:u.mem_write(VT+off,u32(stub))
 u.mem_write(BULLET+0xb0,u32(SOURCE if case['source'] else 0));u.mem_write(BULLET+0x10c,u32(TARGET if case['target'] else 0));u.mem_write(BULLET+0x128,u32(WH));u.mem_write(BULLET+0x6c,u32(case['damage']));u.mem_write(WH+0x15b,b'\1');u.mem_write(TYPE+0x380,struct.pack('<d',case['weight']))
 u.mem_write(BUILDING,u32(VT+0x800));u.mem_write(VT+0x800+0x2c,u32(STUB+64));u.mem_write(BUILDING+0x520,u32(TYPE+0x2000));u.mem_write(TYPE+0x2000+0x16bd,b'\1')
 events=[]
 def hook(uc,pc,size,data):
  if pc==STUB:return returns(uc,case['kind'],0)
  if pc==STUB+16:return returns(uc,case['layer'],0)
  if pc==STUB+32:return returns(uc,case['protected'],0)
  if pc==STUB+48:return returns(uc,TYPE,0)
  if pc==STUB+64:return returns(uc,6,0)
  if pc==0x65ad30:return returns(uc,BUILDING if case['factory'] else 0,4)
  if pc==0x565730:return returns(uc,CELL,4)
  if pc==0x47c520:return returns(uc,BUILDING,0)
  if pc==0x70fee0:events.append('release');return returns(uc,0,4)
  if pc==0x710000:events.append('lift');return returns(uc,0,20)
 u.hook_add(UC_HOOK_CODE,hook);u.reg_write(UC_X86_REG_ESI,BULLET);u.reg_write(UC_X86_REG_EAX,WH);u.reg_write(UC_X86_REG_ESP,SP)
 run_checked(u,0x4694cb,0x469aa4,count=10000)
 return dict(input=case,events=events)

def release(height,clear):
 u=machine();u.mem_map(A,0x20000)
 for ptr in [SOURCE,TARGET]:u.mem_write(ptr,u32(VT))
 u.mem_write(SOURCE+0x2ac,u32(TARGET));u.mem_write(SOURCE+0x21c,u32(123));u.mem_write(TARGET+0x14,b'\4');u.mem_write(TARGET+0x2b0,u32(SOURCE));u.mem_write(TARGET+0x6ad,b'\1')
 for off,stub in [(0x1c8,STUB),(0x3a0,STUB+16),(0x480,STUB+32),(0x3c8,STUB+48)]:u.mem_write(VT+off,u32(stub))
 events=[]
 def hook(uc,pc,size,data):
  if pc==STUB:return returns(uc,height,0)
  if pc==STUB+16:events.append('stun');return returns(uc,0,0)
  if pc==STUB+32:events.append('null_destination');return returns(uc,0,8)
  if pc==STUB+48:events.append('clear_target');return returns(uc,0,4)
 u.hook_add(UC_HOOK_CODE,hook);u.mem_write(SP,u32(STOP)+u32(clear));u.reg_write(UC_X86_REG_ECX,SOURCE);u.reg_write(UC_X86_REG_ESP,SP);run_checked(u,0x70fee0,STOP,count=10000)
 return dict(height=height,clear=clear,events=events,target=read32(u,SOURCE+0x2ac),source=read32(u,TARGET+0x2b0),flags=list(u.mem_read(TARGET+0x425,3)),end=bool(u.mem_read(TARGET+0x6ae,1)[0]),credit=[read32(u,TARGET+0x428)==SOURCE,read32(u,TARGET+0x42c)])

def fall(z,speed):
 u=machine();u.mem_map(A,0x20000);u.mem_write(SOURCE+0xc,u32(TARGET));u.mem_write(TARGET+0x6ad,b'\1');u.mem_write(TARGET+0x9c,struct.pack('<iii',5248,5760,z));u.mem_write(SOURCE+0x28,struct.pack('<f',speed));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,SOURCE);run_checked(u,0x54ca90,0x54cb3e,count=10000)
 sp=u.reg_read(UC_X86_REG_ESP)
 return dict(z=z,speed=speed,new_z=read32(u,sp+0x24),next_crash=read32(u,SOURCE+0x28)&0xffffffff)

def end_gate(moving,stash,swapped,end):
 u=machine();u.mem_map(A,0x20000)
 u.mem_write(SOURCE+4,u32(VT));u.mem_write(SOURCE+0xc,u32(TARGET));u.mem_write(SOURCE+0x94,u32(TYPE if stash else 0));u.mem_write(VT+0x10,u32(STUB));u.mem_write(TARGET+0x6ad,bytes([swapped,end]))
 def hook(uc,pc,size,data):
  if pc==STUB:return returns(uc,moving,4)
 u.hook_add(UC_HOOK_CODE,hook);u.mem_write(SP,u32(STOP)+u32(SOURCE+0x18));u.reg_write(UC_X86_REG_ESP,SP);run_checked(u,0x54db00,STOP,count=1000)
 return dict(moving=moving,stash=stash,swapped=swapped,end=end,admitted=bool(u.reg_read(UC_X86_REG_EAX)&255))

def generate():
 base=dict(source=True,target=True,same=False,previous=False,source_held=False,source_swapped=False,foot=True,swapped=False,kind=1,layer=2,protected=False,factory=False,damage=5000,weight=2.0)
 cases=[base]
 for field,values in dict(source=[False],target=[False],same=[True],previous=[True],source_held=[True],source_swapped=[True],foot=[False],swapped=[True],kind=[2,6,15],layer=[0,1,3,4],protected=[True],factory=[True],damage=[-1,0,1,2,3]).items():
  for value in values:
   cases.append(dict(base,**{field:value}));cases.append(dict(base,previous=True,**{field:value}) if field!='previous' else dict(base,previous=True,target=False))
 return dict(end=[end_gate(m,s,l,e) for m in [False,True] for s in [False,True] for l in [False,True] for e in [False,True]],native_sha256=image_sha256(),admission=[admission(c) for c in cases],release=[release(h,c) for h in [-1,0,1,104,500] for c in [False,True]],fall=[fall(z,s) for z in [-1,0,104,500,999] for s in [1.0,2.5,15.0,25.0,40.0]])
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print({k:len(v) for k,v in data.items() if isinstance(v,list)})
