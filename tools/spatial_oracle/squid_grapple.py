"""Original Squid grapple6297F0, including its actual phase629720 and trigonometry.
Host seams supply weapon/facing/water, allocate animation shells, capture effects,
and record damage without changing health. No audio/device calls run.
"""
import json,struct
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP,UC_X86_REG_ECX,UC_X86_REG_EAX
from tools.projectile_oracle.launch_scatter import machine,u32,read32,returns,SP,RULES
from tools.native_oracle import SCRATCH,run_checked,image_sha256
A=SCRATCH+0x10000;OWNER=A+0x1000;VICTIM=A+0x2000;WEAPON=A+0x3000;WH=A+0x4000;VT=A+0x5000;ANIMS=A+0x10000;STUB=A+0x6000;STOP=A+0x6f00;CELL=A+0x7000;WLIST=A+0x8000
GET_WEAPON=STUB;GET_CELL=STUB+16;DAMAGE=STUB+32;SCHEME=STUB+48;UNINIT=STUB+64

def timeline(facing,policy,frames=155,*,culling=False,rank=0,health=None):
 u=machine();u.mem_map(A,0x30000)
 if health is not None:
  u.mem_write(A+0x48,u32(4));u.mem_write(WH+0x174,bytes([culling]))
  u.mem_write(OWNER+0x150,struct.pack('<f',rank));u.mem_write(VICTIM+0x6c,u32(health))
  u.mem_write(RULES+0x1700,struct.pack('<dd',0.5,0.25));u.mem_write(A+0x9000+0xa0,u32(800))
  for off,stub in [(0x88,STUB+80),(0x84,STUB+80),(0x3b8,STUB+96),(0x3a0,STUB+112)]:u.mem_write(VT+off,u32(stub))
 u.mem_write(A+0x24,u32(OWNER));u.mem_write(A+0x28,u32(VICTIM))
 for ptr in [OWNER,VICTIM]:u.mem_write(ptr,u32(VT))
 for off,stub in [(0x3f8,GET_WEAPON),(0x1bc,GET_CELL),(0x16c,DAMAGE),(0x1e4,SCHEME),(0xf8,UNINIT)]:u.mem_write(VT+off,u32(stub))
 u.mem_write(WLIST,u32(WEAPON));u.mem_write(WEAPON+0xa4,u32(15));u.mem_write(WEAPON+0xac,u32(WH));u.mem_write(WH+0x170,u32(32767));u.mem_write(VICTIM+0x9c,struct.pack('<iii',5248,5760,0))
 u.mem_write(0x8b4154,u32(WLIST+16));u.mem_write(WLIST+16,u32(ANIMS+0x10000));u.mem_write(RULES+0xbd0,u32(2));u.mem_write(RULES+0xbc4,u32(WLIST+32));u.mem_write(WLIST+32,u32(ANIMS+0x11000)+u32(ANIMS+0x12000));u.mem_write(RULES+0x94,u32(ANIMS+0x13000))
 # Native holder vector, roomy enough to avoid host allocation.
 u.mem_write(0xb0f5bc,u32(WLIST+128));u.mem_write(0xb0f5c0,u32(16));u.mem_write(0xb0f5c8,u32(0))
 allrows=[];heap=ANIMS;calls=[];fx=[];hits=[]
 def hook(uc,pc,size,data):
  nonlocal heap
  sp=uc.reg_read(UC_X86_REG_ESP);ecx=uc.reg_read(UC_X86_REG_ECX)
  if pc==STUB+80:return returns(uc,A+0x9000,0)
  if pc==0x62a4a0 and health is not None:
   hits.append('release');uc.mem_write(A+0x28,u32(0));return returns(uc,0,0)
  if pc==STUB+96:
   hits.append('record_kill');return returns(uc,0,4)
  if pc==STUB+112:
   hits.append('stun');return returns(uc,0,0)
  if pc==GET_WEAPON:return returns(uc,WLIST,4)
  if pc==GET_CELL:return returns(uc,CELL,0)
  if pc==0x485060:return returns(uc,1,0)
  if pc==0x427cb0:return returns(uc,0,0)
  if pc==SCHEME:return returns(uc,0,0)
  if pc==0x4c93d0:
   out=read32(uc,sp+4);uc.mem_write(out,u32(facing));return returns(uc,out,4)
  if pc==0x65c7e0:
   lo,hi=read32(uc,sp+4),read32(uc,sp+8);draw=lo if policy=='first' else hi
   calls.append([lo,hi,draw]);return returns(uc,draw,8)
  if pc==0x7c8e17:
   ptr=heap;heap+=0x200;assert heap<ANIMS+0x10000;uc.mem_write(ptr,u32(VT));return returns(uc,ptr,0)
  if pc==0x421ea0:
   typ,coord,delay,loops,flags,z,rev=[read32(uc,sp+4+4*i) for i in range(7)]
   fx.append([typ-(ANIMS+0x10000),list(struct.unpack('<iii',uc.mem_read(coord,12))),delay,loops,flags,z,rev]);return returns(uc,ecx,28)
  if pc==0x424b50:
   uc.mem_write(ecx+0xcc,u32(read32(uc,sp+4)));return returns(uc,0,4)
  if pc==DAMAGE:
   hits.append(read32(uc,read32(uc,sp+4)));return returns(uc,0,28)
  if pc==UNINIT:return returns(uc,0,0)
 u.hook_add(UC_HOOK_CODE,hook)
 for frame in range(frames):
  calls=[];fx=[];hits=[];u.mem_write(0xa8ed84,u32(frame));u.mem_write(SP,u32(STOP));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,A)
  run_checked(u,0x6297f0,STOP,count=100000)
  anim=read32(u,A+0x44)
  allrows.append(dict(frame=frame,state=[read32(u,A+off) for off in [0x48,0x4c,0x50]],rock=read32(u,VICTIM+0x328),anim_frame=read32(u,anim+0xac) if anim else None,paralysis=[read32(u,VICTIM+off) for off in [0x6a0,0x6a8]],draws=calls,fx=fx,hits=hits))
 return dict(facing=facing,policy=policy,steps=allrows)
def generate():return dict(native_sha256=image_sha256(),timelines=[timeline(f,p) for f in [0,8192,16384,32768,57344] for p in ['first','last']])
if __name__=='__main__':
 data=generate();Path(__file__).with_suffix('.json').write_text(json.dumps(data,indent=2)+'\n');print('Executed',sum(len(t['steps']) for t in data['timelines']),'original grapple frames');print([(t['facing'],t['policy'],[s['frame'] for s in t['steps'] if s['hits']]) for t in data['timelines']])
