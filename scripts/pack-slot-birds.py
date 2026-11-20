"""Repack the authorized original Apollo flight frames; requires Pillow."""
import json, plistlib, re
from pathlib import Path
from PIL import Image
from slot_birds_vulture_shadow import remove_front_shadow
root=Path(__file__).resolve().parents[1]
assets=root/'examples/neon-arena/assets'
source=assets/'slot-birds-source'
metadata=plistlib.loads((source/'vulture_anim1.plist').read_bytes())
image=Image.open(source/'vulture_anim1.png').convert('RGBA')
atlas=Image.new('RGBA',(1280,1280))
numbers=lambda value:list(map(int,re.findall(r'-?\d+',value)))
for index in range(16):
    frame=metadata['frames'][f'vulture_FLY_{index+1:02}.png']
    assert not frame['rotated']
    x,y,w,h=numbers(frame['frame'])
    ox,oy,ow,oh=numbers(frame['sourceColorRect'])
    assert (w,h)==(ow,oh) and numbers(frame['sourceSize'])==[320,320]
    tile=Image.new('RGBA',(320,320))
    tile.paste(image.crop((x,y,x+w,y+h)),(ox,oy))
    atlas.paste(tile,((index%4)*320,(index//4)*320))
remove_front_shadow(atlas).save(assets/'slot-bird-flight.png')
