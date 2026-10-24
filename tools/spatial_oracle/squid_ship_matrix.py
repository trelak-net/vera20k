"""Execute Ship69F670 on flat water and compare its matrix to Jumpjet54DCC0.
Only host seams already documented by jumpjet_crash.States are used.
No device/audio code runs. This proves the reused tilt product on flat water,
not Ship slope-transition rendering.
"""
import json, struct
from pathlib import Path
from tools.spatial_oracle import jumpjet_crash as j
from tools.native_oracle import image_sha256

def ship_draw_matrix(row):
    owner = j.States(dict(j.FALL_BASE, **j.TYPES['DISK'], order=None, phase=2,
        moving=False,start=[*j.centre(10),500],target_height=750))
    u=owner.uc
    # ILocomotion points four bytes into the object. Its +18/+1C slope words,
    # +20 timer and +2C duration are all zero: no slope and no interpolation.
    u.mem_write(j.LOCO+0x1C,bytes(0x18))
    u.mem_write(j.TYPE+0x360,struct.pack('<dd',*row['half_sizes']))
    u.mem_write(j.OWNER+0x328,struct.pack('<ff',*row['angles']))
    facing=j.OWNER+0x388
    owner.call(j.FACING_CONSTRUCTOR,facing,[])
    owner.call(j.FACING_SET_ROT,facing,[4])
    u.mem_write(j.SCRATCH+0xEB00,j.dwords(row['facing']))
    owner.call(j.FACING_SET,facing,[j.SCRATCH+0xEB00])
    out,key=j.SCRATCH+0xEB80,j.SCRATCH+0xEBF0
    u.mem_write(key,j.dwords(row['key']))
    owner.call(0x69F670,0,[j.LOCO+4,out,key])
    return dict(matrix=[f'{v:08x}' for v in struct.unpack('<12I',u.mem_read(out,48))],key=owner.read32(key))

def generate():
    rows=[]
    for name,row in j.draw_matrix_rows():
        if row['tilt']!=1:continue
        ship=ship_draw_matrix(row)
        assert ship['matrix']==j.draw_matrix(row)['matrix'],name
        rows.append(dict(name=name,input=row,output=ship))
    return dict(native_sha256=image_sha256(),rows=rows)

if __name__=='__main__':
    result=generate()
    Path(__file__).with_suffix('.json').write_text(json.dumps(result,indent=2)+'\n')
    print('Original flat-water Ship and Jumpjet tilt match:',len(result['rows']),'cases')
