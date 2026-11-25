# Rapport: Alt 1401 (fåror som objekt) + Alt 1405 (Gransens ankarvärden)

Skriven 2026-11-21 av Cursor i arbetsträdet `pi-hckarta4`, gren `hckarta-tid1401`.
Underlag: `/home/hakan/sandbox/pi-hckarta4/n10/docs/2026-11-21-lasunderlag-kartinterp.md`
(skrivet före bygget; tal och gränser oförändrade).

## Git

```
git rev-parse HEAD (kodcommit): 3374376b996ab89c0490f744220b78a71da0dce4
git rev-parse HEAD (bas före uppdraget): 57986b0546bcd7c2f1e7c82d0e5b282af978b3d6
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
 M n10/hckarta/src/main.rs
?? n10/hckarta/tests/tid1401.rs
?? n10/hckarta/tid1401-fore.txt
?? n10/hckarta/tid1401-rapport.md
```

## Vad som byggdes

### A. Ankarvärden (Alt 1405)

`flodregler(Period::Gransen)`:

| Fält | före | efter | skäl |
|------|-----:|------:|------|
| `bredd_rutor` | 3 | **2** | Butler 1980 [A]: bredden minskade |
| `trask_avstand` | 2 | **3** | Hasler och Renne 2025 [A]; talet 3 = eget val |
| `sjoar_per_100_rutor` | 1,0 | **2,0** | samma källa; talet 2,0 = eget val |

Kommentaren om Tanis («något bredare fåra») borttagen. Aldre och Ovre oförändrade.

### B. Fåror som objekt (Alt 1401)

- `karta_for_ar` / `--tidsmodell 2`: Aldres mittlinjer byggs en gång; från Ovre lever två
  ur `fro ^ 0x4C45_5646_4152_4F52`; Gransen behåller samma två; övergivna mittlinjerutor →
  `Trask`, `vatten_yta = höjd`.
- `nasta_andring(tt, t)`, `levande_foror_nr`, `mittlinjer_aldre`.
- `karta_full` / `karta_full_version` (tidsmodell 1) oförändrade för t = 500 000 och 20 000;
  t = 0 ändras bara via A.

## Anpassning av befintliga prov

Inga påståenden i `tests/s*.rs`, `fara1343.rs` eller `bredd1386.rs` behövde ändras:
de prövar t = 500 000 / 20 000 (eller periodnamn utan Gransens ankarvärden). Avsnitt A påverkar
bara `karta_full` vid t = 0, som inget befintligt prov låste.

## Prov 1 — gamla kartor orörda utom Gransen (sha256)

Före: `n10/hckarta/tid1401-fore.txt`. Efter: byte-identiska filer för t = 500000 och 20000,
flodversion 1 och 2 (`--fro 181`, `--skriv`).

| flodversion | t | kontrollsumma |
|------------:|--:|--:|
| 1 | 500000 | 9828389463751256610 |
| 1 | 20000 | 7307636836510803732 |
| 2 | 500000 | 10464361137633183643 |
| 2 | 20000 | 3941459844937214511 |

### t = 0 (ändras genom A), tidsmodell 1

| | Land | Flodfara | Trask | Sjo | kontrollsumma |
|--|--:|--:|--:|--:|--:|
| v1 före | 2603 | 350 | 88 | 31 | 5993398614623432911 |
| v1 efter | 2657 | 256 | 98 | 61 | 9344846268250959607 |
| v2 före | 2936 | 0 | 105 | 31 | 3298420734369083130 |
| v2 efter | 2899 | 0 | 112 | 61 | 12928887189391796935 |

## Prov 2 — hcsim orört

Kommando: `hcsim kor --lat0 47 --lon0 -108 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 181 --ar 5 --n0 400`

| | före | efter |
|--|--|--|
| slut_antal | 4992 | 4992 |
| djur_summa | 9527944887746424482 | 9527944887746424482 |
| karta_summa | 9828389463751256610 | 9828389463751256610 |
| fodda / dod_svalt / dod_alder | 27785 / 23125 / 68 | samma |
| v_medel | 82.54336761406384 | samma |

## Redovisade tal (`--tidsmodell 2`, fro 181, 64×48, ruta_m=2000)

Σ `fara_andel` / 3072 = medel (version 2).

### Version 1

| t | period | levande | Land | Flodfara | Trask | Sjo |
|--:|--|--|--:|--:|--:|--:|
| 500000 | Aldre | 0 1 2 3 4 | 2225 | 640 | 146 | 61 |
| 290000 | Ovre | 1 3 | 2501 | 256 | 276 | 39 |
| 20000 | Ovre | 1 3 | 2501 | 256 | 276 | 39 |
| 0 | Gransen | 1 3 | 2494 | 256 | 269 | 53 |

### Version 2

| t | period | levande | Land | Flodfara | Trask | Sjo | medel fara_andel |
|--:|--|--|--:|--:|--:|--:|--:|
| 500000 | Aldre | 0 1 2 3 4 | 2822 | 0 | 189 | 61 | 0.011903 |
| 290000 | Ovre | 1 3 | 2742 | 0 | 286 | 44 | 0.005210 |
| 20000 | Ovre | 1 3 | 2742 | 0 | 286 | 44 | 0.005210 |
| 0 | Gransen | 1 3 | 2736 | 0 | 279 | 57 | 0.005210 |

Bredder (v2, ursprungliga nummer): fåra 1 = 149,20 m; fåra 3 = 238,42 m (samma vid 290000/20000/0).

## Röda mutanter (prövade med flit i kopia, sedan återställda)

| Mutant | Prov som blev rött |
|--------|-------------------|
| Ovre ritas om med Ovres regler (`karta_for_ar` → `karta_full_version`) | prov 4 |
| Övergivna rutor blir `Land` i stället för `Trask` | prov 6 |
| Alltid fåra 0 och 1 | prov 10 |

## cargo test

hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`): alla gröna —
bredd1386 11, fara1343 10, granser 1, s1 5, s2 6, s3 5, s4 8, tid1401 12.

hcsim: slutsummor oförändrade (se prov 2); ingen kodändring i `n10/hcsim/`.

## Avvikelser

Inga. Underlagets tal och gränser oförändrade.
