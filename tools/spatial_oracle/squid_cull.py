"""Original6297F0 culling, executing HealthStatus5F5DD0 and IsElite750010.
ExitUnit is a host seam clearing the victim; RecordKill and Stun are observed.
The Rust retail world separately checks actual release, sinking and cleanup.
"""
import json
from pathlib import Path
from tools.spatial_oracle.squid_grapple import timeline
from tools.native_oracle import image_sha256

def generate():
    rows=[]
    for culling in [0,1]:
        for rank in [0,1,2]:
            for health in [0,1,199,200,201,400,401]:
                result=timeline(0,'first',1,culling=culling,rank=rank,health=health)
                rows.append(dict(culling=culling,rank=rank,health=health,output=result['steps'][0]))
    return dict(native_sha256=image_sha256(),rows=rows)
if __name__=='__main__':
    result=generate()
    Path(__file__).with_suffix('.json').write_text(json.dumps(result,indent=2)+'\n')
    print('Executed',len(result['rows']),'original culling controls')
