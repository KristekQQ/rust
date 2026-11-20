"""Remove only the original vulture FLY atlas's exterior neutral drop shadow.

The 16 original 320px front cells are the only supported input region. Original
source sheets remain untouched. Colored artwork and enclosed dark details are
preserved; only exterior-connected, near-neutral dark pixels lose alpha.
"""
from collections import deque


def remove_front_shadow(image):
    image = image.convert("RGBA").copy()
    assert image.width >= 1280 and image.height >= 1280
    pixels = image.load()
    for row in range(4):
        for column in range(4):
            pending = deque([(0, 0)])
            visited = set()
            while pending:
                x, y = pending.popleft()
                if not (0 <= x < 320 and 0 <= y < 320) or (x, y) in visited:
                    continue
                visited.add((x, y))
                position = (column * 320 + x, row * 320 + y)
                r, g, b, alpha = pixels[position]
                high, low = max(r, g, b), min(r, g, b)
                # Apollo shadow palette: dark, almost neutral; purple feathers
                # are saturated and block the exterior flood, including edges.
                if alpha and not (high <= 60 and (high - low) * 100 <= high * 40):
                    continue
                if alpha:
                    pixels[position] = (r, g, b, 0)
                pending.extend(((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)))
    return image
