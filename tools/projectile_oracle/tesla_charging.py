"""BuildingClass::UpdateOverpower 450590 with live original power-ratio math.
No function stubs: charger target pointers and house power are supplied state.
"""
import json,struct
from pathlib import Path
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,PARENT,SP
from tools.native_oracle import SCRATCH,run_checked,image_sha256
HOUSE=SCRATCH+0x20000
VECTOR=SCRATCH+0x30000
TROOPS=SCRATCH+0x40000
STOP=SCRATCH+0x60000

def run(case):
 uc=machine()
 uc.mem_map(SCRATCH+0x10000,0x60000)
 uc.mem_write(PARENT+0x21c,u32(HOUSE))
 uc.mem_write(PARENT+0x660,bytes([case['online'],1]))
 uc.mem_write(PARENT+0x670,u32(VECTOR))
 uc.mem_write(PARENT+0x67c,u32(len(case['targeting'])))
 uc.mem_write(HOUSE+0x53a4,u32(case['power']))
 uc.mem_write(HOUSE+0x53a8,u32(case['drain']))
 for i,targeting in enumerate(case['targeting']):
  obj=TROOPS+i*0x1000
  uc.mem_write(VECTOR+i*4,u32(obj))
  uc.mem_write(obj+0x2b4,u32(PARENT if targeting else 0))
 uc.mem_write(SP,u32(STOP))
 uc.reg_write(UC_X86_REG_ECX,PARENT);uc.reg_write(UC_X86_REG_ESP,SP)
 run_checked(uc,0x450590,STOP,count=10000)
 count=read32(uc,PARENT+0x67c)
 return dict(input=case,remaining=[(read32(uc,VECTOR+i*4)-TROOPS)//0x1000 for i in range(count)],overpowered=bool(uc.mem_read(PARENT+0x661,1)[0]))

def generate():
 rows=[]
 for online in (0,1):
  for power,drain in ((0,0),(0,100),(50,100),(100,100),(150,100)):
   for targets in ([],[True],[True,True],[True,True,True],[False,True,False,True],[True,False,False,True,True]):
    rows.append(run(dict(online=online,power=power,drain=drain,targeting=targets)))
 return dict(native_sha256=image_sha256(),entry='0x00450590',rows=rows)
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print('Verified',len(data['rows']),'original Tesla state cases')
