# Rapport: Alt 1372/1472 (flodernas riktning mot sydost)

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta7`, gren `hckarta-riktning1472`.
Underlag: `/home/hakan/sandbox/pi-hckarta7/n10/docs/2026-11-22-lasunderlag-kartriktning.md`
(skrivet före bygget; tal och gränser oförändrade).

## Git

```
git rev-parse HEAD (kodcommit): 2a4e84519dc5f207eb4c30ae40c0663b62885f51
git rev-parse HEAD (bas före uppdraget): fd252965de451240f79d37fc7b2e31c2a568ddfa
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
 M n10/hckarta/src/main.rs
 M n10/hckarta/tests/bredd1386.rs
 M n10/hckarta/tests/fara1343.rs
 M n10/hckarta/tests/tid1401.rs
 M n10/hckarta/tests/vall1371.rs
 M n10/hckarta/tests/vaxt1433.rs
?? n10/hckarta/riktning1472-fore.txt
?? n10/hckarta/tests/data/riktning1472-fore-v*-tm*-t*/
?? n10/hckarta/tests/riktning1472.rs
?? n10/hckarta/riktning1472-rapport.md
```

## Vad som byggdes

- `--flodriktning 2` (standard 1): steg öst/sydost/söder, riktningsstraff k på söder,
  starter längs västra+norra kanten, slut vid öst/syd-kant eller hav,
  `L = ruta_m · √(di² + dj²)` (sista rutan: `L = ruta_m`).
- `karta_full_version` / `karta_for_ar` / `lagg_flodnat_version` fick parametern `flodriktning`.
- `karta_full` (hcsim) ger alltid riktning 1.
- `k` = 0 (minsta i {0,2,4,8,16,32} med medel i 125–140°; alla gav 132,0°).
- LASMIG: Hasler och Renne [A], Butler [A]; 125–140°, k, norra kanten, Ovre = eget val.

## Anpassning av befintliga prov

| Fil | Ändring | Varför |
|-----|---------|--------|
| `tests/{fara1343,bredd1386,tid1401,vall1371,vaxt1433}.rs` | `, 1` som sista argument till `karta_full_version` / `karta_for_ar` | ny API-parameter; beteende = riktning 1 |

Inga assertions i befintliga prov ändrades.

## Prov 1 — riktning 1 orörd (sha256)

Före: `n10/hckarta/riktning1472-fore.txt`. Efter: samma hashar byte för byte för
`--t 500000/20000/0`, version 1 och 2, båda `--tidsmodell 1|2`, utan `--flodriktning` och med
`--flodriktning 1`, `--fro 181`, `--skriv`.

Kontrollsummor (version 1, tidsmodell 1) oförändrade: 9828389463751256610 / 7307636836510803732 /
9344846268250959607.

## Prov 2 — hcsim orörd

Kommando: `hcsim kor --lat0 47 --lon0 -108 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 181 --ar 5 --n0 400`

| | före | efter |
|--|--|--|
| slut_antal | 4992 | 4992 |
| djur_summa | 9527944887746424482 | 9527944887746424482 |
| karta_summa | 9828389463751256610 | 9828389463751256610 |
| fodda / dod_svalt / dod_alder | 27785 / 23125 / 68 | samma |
| v_medel | 82.54336761406384 | samma |

## Redovisade tal (räknade av programmet)

Mätsätt: första→sista ruta, 0° = norr, 90° = öst (samma som `kartriktning-fore.rs`).

| | n | medel | median | min | max |
|--|--:|--:|--:|--:|--:|
| riktning 1 Aldre (frö 1–50) | 250 | 103,1° | 103,4° | 86,4° | 116,9° |
| riktning 2 Aldre | 250 | 132,0° | 130,5° | 99,0° | 168,0° |
| riktning 1 Ovre | 100 | 99,2° | 106,8° | 86,4° | 111,6° |
| riktning 2 Ovre | 100 | 131,7° | 149,2° | 112,4° | 151,0° |

Valt **k = 0**. Medel per prövat k (Aldre, frö 1–50): alla 0…32 → **132,0°** (söder väljs bara när Ö/SO saknas, typiskt längs östra kanten).

Slutplatser riktning 2 Aldre (250 fåror): östra kanten **250**, södra kanten **0**, hav **0**.

Frö 181, t = 500 000, antal rutor per vattentyp:

| riktning | version | Land | Flodfara | Trask | Sjo | Hav |
|---------:|--------:|-----:|---------:|------:|----:|----:|
| 1 | 1 | 2225 | 640 | 146 | 61 | 0 |
| 1 | 2 | 2822 | 0 | 189 | 61 | 0 |
| 2 | 1 | 2002 | 476 | 533 | 61 | 0 |
| 2 | 2 | 2456 | 0 | 555 | 61 | 0 |

## Röda proven (mutanter i kopia)

| Mutant | Prov | Utfall |
|--------|------|--------|
| Steget (0,1) borttaget ur STEG (underlagets «k=1000 / inga söderssteg») | prov 3 | rött: medel 112,0° ∉ 125–140° |
| Steget (1,−1) tillåtet i stället för (0,1) | prov 4 | rött: «otillåtet steg (1,−1)» |
| Starter mid-map i stället för V+N-kant | prov 5 | rött: «start (32,24) inte på västra/norra kanten» |

Obs: k = 1000 som enbart straff ändrar **inte** medelriktningen på denna DEM (söder bara när ensam kandidat). Underlagets avsikt «inga söderssteg» verifierades genom att ta bort södersteget ur stegmängden.

## Avvikelser

1. **Johnsons kvot med riktning 2** (underlaget prov 7 / Alt 1433): underlaget kräver &lt; 0,10 för frö 181.
   Uppmätt: Aldre **0,132**, Ovre **0,120**. Orsak: fler Trask-rutor längs längre sydostfåror
   (barrträd 0,25 i träsk). Underlagets gräns 0,10 **ändrades inte**. Riktning 1 behåller kvot &lt; 0,10
   (befintligt vaxt1433-prov grönt). Prov 7 låser Alt 1401 (två levande) och Alt 1371 (bälte+vall ≤ 1);
   Johnson redovisas här.
2. **hcsim `spara1302::kontroll7`**: misslyckades med diff i `n10/hcsim/.gitignore` (utanför tillåtna
   filer; rör inte kartsummorna). Prov 2:s slutsummor oförändrade.

## cargo test

hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`): alla integrationsprov gröna, inkl.
`riktning1472` 9 passed; befintliga fara/bredd/tid/vall/vaxt oförändrade i beteende.

hcsim: lib-tester 54 passed; `spara1302` 12 passed, 1 failed (kontroll7, se ovan), 2 ignored.
Kontroll1 (dagens körning oförändrad) ok.

## Tillägg vid Claudes granskning (2026-11-22)
- Prov 6 räknade bara formeln i provet och anropade aldrig programmet; felet "L klipps till ruta_m" gav inget rött i prov 6 (bara i prov 1). Provet läser nu
  kartans `fara_andel` i rutor där en fåra tar steget sydost ensam (frö 181, Aldre, riktning 2) och jämför med w · 2000 · √2 / 2000². Prövat: felet ger
  rött i prov 6.
- hcsim:s `spara1302::kontroll7` föll här för att grenen skapades före BRAIN:s grenspärr (6f7eaa3); efter ihopslagningen gäller main:s version.
- Johnsons kvot över 0,10 med riktning 2 (avvikelse 1) och träsken (555 mot 189 rutor, frö 181, Aldre, version 2) är ett öppet val; riktning 2 är inte
  standard.

