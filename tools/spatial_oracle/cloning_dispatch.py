"""Original infantry clone loop4449D9..444A53; supplied CreateInstance/ExitObject.
This establishes factory/clone gates, count, owner and list order, not placement.
"""
import json
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ESI,UC_X86_REG_EDI,UC_X86_REG_EBX,UC_X86_REG_EAX,UC_X86_REG_ECX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP
from tools.native_oracle import SCRATCH,run_checked,image_sha256
BASE=SCRATCH+0x10000
PRODUCER=BASE;TYPE=BASE+0x1000;HOUSE=BASE+0x3000;PRODUCT=BASE+0x4000
VT=BASE+0x5000;IVT=BASE+0x6000;ARRAY=BASE+0x7000;VATS=BASE+0x8000
GET=BASE+0xf000;CREATE=GET+16;EXIT=GET+32

def case(factory,cloning,count):
 u=machine();u.mem_map(BASE,0x10000)
 for address,value in [(PRODUCER+0x520,TYPE),(PRODUCER+0x21c,HOUSE),(TYPE+0xeb8,factory),(PRODUCT,VT),(VT+0x84,GET),(IVT+0x8c,CREATE),(HOUSE+0xfc,ARRAY),(HOUSE+0x108,count),(VT+0x100,EXIT)]: u.mem_write(address,u32(value))
 u.mem_write(TYPE+0x16ac,bytes([cloning]))
 for i in range(count):
  vat=VATS+0x800*i
  for address,value in [(ARRAY+4*i,vat),(vat,VT),(vat+0x21c,0x1230+i)]:u.mem_write(address,u32(value))
 calls=[]
 def hook(uc,pc,size,data):
  sp=uc.reg_read(UC_X86_REG_ESP)
  if pc==GET: returns(uc,IVT+0x800,0)
  elif pc==CREATE:
   calls.append(["create",read32(uc,sp+4)]);returns(uc,0x5670+len(calls),4)
  elif pc==EXIT:
   calls.append(["exit",(uc.reg_read(UC_X86_REG_ECX)-VATS)//0x800,read32(uc,sp+4)]);returns(uc,2,8)
 u.mem_write(IVT+0x800,u32(IVT));u.hook_add(UC_HOOK_CODE,hook)
 for reg,val in [(UC_X86_REG_ESP,SP),(UC_X86_REG_ESI,PRODUCER),(UC_X86_REG_EDI,PRODUCT),(UC_X86_REG_EBX,16)]:u.reg_write(reg,val)
 run_checked(u,0x4449d9,0x444a53,count=10000)
 expected=count if factory==16 and not cloning else 0
 assert len(calls)==2*expected
 assert [c[1] for c in calls if c[0]=="exit"]==list(range(expected))
 assert [c[1] for c in calls if c[0]=="create"]==[0x1230+i for i in range(expected)]
 return dict(factory=factory,cloning=cloning,count=count,calls=calls)
if __name__=='__main__':
 data=dict(native_sha256=image_sha256(),cases=[case(f,c,n) for f in [-1,6,16,40] for c in [0,1] for n in [0,1,2,3,8]])
 Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n')
 print('Original cloning dispatch:',len(data['cases']),'cases passed')
