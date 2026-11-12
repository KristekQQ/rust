# Původ textur

Vytvořeno 2026-10-18 vestavěným nástrojem ImageGen podle uživatelovy reference
neonové arény. Výchozí reference obsahovala hráče, ovoce a herní HUD; generované
prostředí je neobsahuje. Nejde o skutečné fotografie ani fotogrammetrii.

`arena-color-source.png` je barevný zdroj, `arena-emission-source.png` jeho emisní
interpretace na černém podkladu. Emisní obrázek je samostatně generovaný; přesné
pixelové zarovnání všech tenkých světel není zaručeno. Oba zdroje mají poměr 2:1.
`build-cubemap.mjs` používá fasádovou část obou zdrojů jako texturu válcového proxy (poloměr7m, výška12m, oči1.65m). Vypéká šest 90° pohledů 1024×1024; perspektivní podlahu z ilustrace vynechává. Zdroj není považován za validní equirectangular panorama. Jde o přibližnou kalibraci, ne obnovenou geometrii fotografie.
Původní environment varianta načítala `color-{px,nx,py,ny,pz,nz}.png` a odpovídající `emission-*`. Aktuální černá herní scéna je nenačítá; jde o archivované podklady.

## Zadání barevného zdroje (ImageGen)

Create a production game environment texture asset using the attached arena image ONLY as visual style reference. Output exactly one seamless full 360 x 180 degree equirectangular panorama, 2:1 aspect ratio, ideally 4096x2048 or 2048x1024. This will be mapped to an immersive VR environment, NOT displayed as a flat illustration: correct latitude-longitude projection, horizon exactly middle row, left and right edges seamlessly join, zenith up at top and nadir down at bottom. Camera at eye height 1.65m in the exact center of a circular futuristic luxurious arena 14m radius. Architectural setting matching reference: three circular balcony levels, dark midnight blue reflective metal and glass, curved cyan neon strips, vertical magenta neon ribs, pink violet glowing arches, sparse orange lamps, distant palms through the arch openings, open starfield and luminous blue dome above. The entire panorama is rich, sharply detailed textured architecture and realistic material lighting, vibrant cyan and magenta with deep dark contrasts, cinematic like the reference. Empty arena: remove all people, fruit, bombs, holographic HUDs, floating game objects, writing, signs, logos, and all text. Floor has polished dark tiles with embedded cyan and magenta concentric rings and reflections. Keep neon tubes sharply defined with restrained bloom because runtime will add glow using a separate emission map. Perfectly seamless continuous surrounding room, not a repeated image, not a collage, no border. Final deliverable is the texture image itself, no labels or presentation mockup.

## Zadání emisního zdroje (ImageGen)

Technical texture edit: produce the RGB EMISSIVE MAP corresponding EXACTLY pixel for pixel to this source texture, same 2:1 aspect ratio and resolution, no camera/geometry/layout changes at all. Black out every non-emitting surface: all metal architecture, tiled floor, palms, glass, sky, stars, atmospheric haze, reflected lights on floor and balconies. Preserve ONLY directly luminous neon tubes as sharp colored lines in their original exact positions (cyan balcony strips, magenta vertical ribs and arches, central floor cyan/magenta rings), plus warm orange lamp cores. These must retain their source cyan/magenta/orange RGB hues on pure black. No glow halos/bloom (runtime adds it), no reflections, no background, no text, no reimagining or new detail. This is a separate emission texture used with the unchanged original base-color image, so spatial alignment of all emitting elements is essential. Entire output opaque black except original light sources. Return map only.

## Žula pro 3D památník

`granite.png` vznikl 2026-10-18 vestavěným ImageGen. Je použit jako samostatný
glTF PBR base-color materiál; obsah není zapečený do kamene. Modely monumentu a
rámečku displeje vznikají offline v `build-monument.mjs`. Displej žulovou
texturu nepotřebuje a samostatně ji nenačítá.

Zadání ImageGen:

Production game PBR base-color texture, seamless square 1024x1024 tile of finely honed dark charcoal blue-grey granite for a futuristic memorial stone in a neon arena. Orthographic straight-on flat surface scan, no perspective, no lighting direction, no cast shadows, no objects, no edges, no text, no symbols, no engraving. Dense realistic very fine crystalline stone grain, tiny subtle silver flecks, broad faint mineral variation, subdued dark tones rather than pure black, matte naturally weathered but carefully polished stone. Low contrast suitable behind cream and warm gold bitmap lettering. Perfectly seamless all edges, albedo only, no neon baked in, no glow. The entire image is material texture.
