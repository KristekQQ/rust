# Střelnice jablek

Spuštění přes HTTP: `scripts/run-local.sh`, potom `/examples/projectiles/`.
48 předem vytvořených jablek se recykluje; zaparkované kusy nejsou aktivní
ve fyzice. Reset využívá existující pool. F3 přepíná informace rendereru,
tlačítko „Otestovat srážku“ vyšle dvojici proti sobě.

Fyzika je Rust/WASM na CPU, WebGPU slouží pro vykreslení. Kontakty používají
kulovou obálku podle měřítka, stejné hmotnosti, restituci 0,75 a oddělení
překrytí. Nejde o přesné kontakty trojúhelníkových povrchů.
Simulace používá kroky nejvýše 1/240 s; po dlouhé pauze zpracuje nejvýše 0,25 s.

Výpočet pracuje se souvislým znovupoužitým polem těles. Do 16 aktivních kusů
kontroluje všechny páry, větší skupiny používají sweep podle X a filtr Y/Z.
Kandidáti i pomocná pole zůstávají alokované pro další snímky.
Každý krok má nejvýše čtyři korekční iterace s obnovou kandidátů:
oddělení jednoho páru může vytvořit další kontakt. Při nulových korekcích
se iterace ihned ukončí.
V hustém shluku může hledání párů stále dosáhnout kvadratické složitosti.

## Lokální měření 2026-10-17

`cargo test --release benchmark_projectile_solver -- --ignored --nocapture --test-threads=1`

Nativní release, medián pěti vzorků, každý 240 kroků scény při 60 Hz,
po jednom zahřívacím snímku. Stejné počáteční pozice/rychlosti, započítaná
příprava těles i zpětný zápis transformací; bez vykreslování a JS.

| Aktivní tělesa | Před úpravou, µs/snímek | Po úpravě, µs/snímek |
|---|---:|---:|
| 8 | 1,434 | 1,253 |
| 48 | 64,624 | 10,021 |
| 256 | 4 831,945 | 187,671 |

Tato čísla nejsou browserová/WASM měření, FPS ani porovnání s WebGPU compute.
Benchmark má specifické rozložení těles; nepředstavuje všechny hustoty a trajektorie.
Nový solver navíc řeší následné kontakty vzniklé oddělením jiného páru;
baseline měl pouze jediný korekční průchod na krok.
