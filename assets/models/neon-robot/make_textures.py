"""Run in Blender's bundled Python: deterministic tiled worn-metal PBR images."""
from pathlib import Path
import struct
import zlib
import numpy as np

ROOT = Path(__file__).resolve().parent / 'textures'
ROOT.mkdir(exist_ok=True)
N = 512
rng = np.random.default_rng(4182)
y, x = np.mgrid[:N, :N]
# Periodic machining marks and multi-scale, spatially correlated wear.
grain = .016 * np.sin(y * 2.93 + np.sin(x * .037))
noise = rng.normal(0, .009, (N, N))
macro = .018 * (np.sin(x * 2*np.pi/N * 7) * np.cos(y * 2*np.pi/N * 11))
scratch = np.zeros((N, N))
for _ in range(145):
    a, b = rng.integers(N, size=2)
    length = rng.integers(14, 145)
    u = np.arange(length)
    slope = rng.uniform(-.18, .18)
    xs = (a + u) % N
    ys = (b + np.round(u * slope).astype(int)) % N
    scratch[ys, xs] += rng.uniform(.035, .1) * np.sin(np.pi*u/length)**.7
pit = np.zeros((N, N))
for _ in range(900):
    a, b = rng.integers(N, size=2)
    pit[b,a] = rng.uniform(.03, .12)
    pit[(b+1)%N,a] = pit[b,a] * .35
height = grain*.2 + noise*.13 + scratch*.22 - pit*.28
dx = (np.roll(height, -1, axis=1) - np.roll(height, 1, axis=1))*3
dy = (np.roll(height, -1, axis=0) - np.roll(height, 1, axis=0))*3
normal = np.stack([-dx,-dy,np.ones_like(dx)],axis=-1)
normal /= np.sqrt((normal*normal).sum(axis=-1))[...,None]
normal = normal*.5+.5

def png(path, data):
    pixels = np.round(np.clip(data, 0, 1)*255).astype(np.uint8)
    if pixels.ndim == 2: pixels = np.repeat(pixels[...,None], 3, axis=-1)
    raw = b''.join(b'\0'+row.tobytes() for row in pixels)
    def chunk(kind, payload):
        return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload)&0xffffffff)
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',N,N,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw,9))+chunk(b'IEND',b''))

for name, color, rough in [('violet',(.36,.28,.39),.32),('brass',(.74,.48,.22),.28)]:
    value=1+grain+macro+noise+scratch*1.6-pit*2
    base=value[...,None]*np.array(color)
    # Exposed silver machining scratches on violet; warmer wear on brass.
    base += scratch[...,None]*np.array((.9,.94,1) if name=='violet' else (.7,.5,.15))
    png(ROOT/f'{name}_base.png',base)
    png(ROOT/f'{name}_rough.png',rough+pit*2+noise*.6-scratch*.65)
    png(ROOT/f'{name}_normal.png',normal)
print('Worn metal PBR: six 512px tiled images written to', ROOT)
