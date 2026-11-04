from pathlib import Path
import math
import struct
import sys

def near_segment(x, y, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0, min(1, ((x - a[0])*dx + (y - a[1])*dy)/(dx*dx + dy*dy)))
    return math.hypot(x - a[0] - t*dx, y - a[1] - t*dy) < 1.45

def icon(size, done):
    path = [(7, 16), (13, 22), (24, 10)] if done else [(7, 17), (8, 11), (14, 8), (21, 10), (23, 15), (19, 13)]
    if not done:
        path += [(23, 15), (24, 10)]
    pixels = bytearray()
    for row in reversed(range(size)):
        for column in range(size):
            x, y = (column + .5)*32/size, (row + .5)*32/size
            r = math.hypot(x - 16, y - 16)
            if r > 15:
                pixels.extend((0, 0, 0, 0))
            elif r > 13.2 or any(near_segment(x, y, a, b) for a, b in zip(path, path[1:])):
                pixels.extend((255, 255, 255, 255))
            else:
                pixels.extend((85, 152, 33, 255) if done else (214, 105, 36, 255))
    mask = bytes(((size + 31)//32)*4*size)
    image = struct.pack('<IIIHHIIIIII', 40, size, size*2, 1, 32, 0, len(pixels), 0, 0, 0, 0) + pixels + mask
    return struct.pack('<BBBBHHII', size, size, 0, 0, 1, 32, len(image), 22), image

out = Path(sys.argv[1])
for name, done in [('pending', False), ('complete', True)]:
    entry, image = icon(32, done)
    (out / (name + '.ico')).write_bytes(struct.pack('<HHH', 0, 1, 1) + entry + image)
