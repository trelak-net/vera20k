"""Execute BulletClass::Fire's Inviso/FlakScatter placement, 46872E..4688A9.
Only RandomRanged is substituted: arguments and returned values are recorded.
Sqrt_Approx, sin/cos and ftol execute the installed original instructions.
"""
import json, struct
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_EBX,UC_X86_REG_EDI,UC_X86_REG_ECX,UC_X86_REG_EAX,UC_X86_REG_EDX
from tools.projectile_oracle.launch_scatter import machine, u32, read32, returns, RULES, PARENT, BULLET_TYPE, WEAPON, TARGET, SP, RANDOM_RANGED
from tools.native_oracle import run_checked, image_sha256

def run(case):
 uc=machine()
 uc.mem_write(RULES+0x1734,u32(case['scatter']))
 uc.mem_write(PARENT+0xac,u32(BULLET_TYPE))
 uc.mem_write(PARENT+0x130,u32(WEAPON))
 uc.mem_write(WEAPON+0xb4,u32(case['range']))
 uc.mem_write(BULLET_TYPE+0x2a3,bytes([case['flak']]))
 uc.mem_write(BULLET_TYPE+0x29e,bytes([case['inviso']]))
 uc.mem_write(TARGET,struct.pack('<iii',*case['origin']))
 uc.mem_write(SP+0x44,struct.pack('<iii',*case['target']))
 x,y,z=case['target']
 for reg,val in ((UC_X86_REG_ESP,SP),(UC_X86_REG_EBX,PARENT),(UC_X86_REG_EDI,TARGET),(UC_X86_REG_ECX,x),(UC_X86_REG_EAX,y),(UC_X86_REG_EDX,z)):
  uc.reg_write(reg,val&0xffffffff)
 draws=iter(case['draws']);calls=[]
 def hook(_uc,address,_size,_data):
  if address==RANDOM_RANGED:
   sp=uc.reg_read(UC_X86_REG_ESP)
   calls.append([read32(uc,sp+4),read32(uc,sp+8)])
   returns(uc,next(draws),8)
 uc.hook_add(UC_HOOK_CODE,hook)
 run_checked(uc,0x46872e,0x4688a9,count=200000,required_addresses=[0x4cac40,0x4cacb0,0x4cad00] if case['inviso'] and case['flak'] else [])
 assert next(draws,None) is None
 assert uc.reg_read(UC_X86_REG_ESP)==SP
 return dict(input=case,calls=calls,result=list(struct.unpack('<iii',uc.mem_read(SP+0x44,12))))

def generate():
 rows=[]
 for origin,target in (([10240,12800,128],[12032,14080,1024]),([0,0,0],[0,0,0]),([-1337,5001,-128],[8,-2500,2048]),([0,0,0],[16777217,1,104]),([12032,14080,1024],[10240,12800,128])):
  for scatter,range_ in ((256,3072),(123,512),(0,256)):
   for draw,angle in ((0,0),(scatter,0x1fffffff),(scatter*2,0x7ffffffe),(scatter//2,0x40000000)):
    rows.append(run(dict(origin=origin,target=target,scatter=scatter,range=range_,inviso=1,flak=1,draws=[draw,angle])))
 for inviso,flak in ((0,0),(0,1),(1,0)):
  rows.append(run(dict(origin=[10240,12800,128],target=[12032,14080,1024],scatter=256,range=3072,inviso=inviso,flak=flak,draws=[])))
 return dict(native_sha256=image_sha256(),entry='0x0046872e..0x004688a9',rows=rows)
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print('Verified',len(data['rows']),'original scatter cases')
