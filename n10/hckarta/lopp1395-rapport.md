# Rapport: Alt 1395 (floderna byter lopp över tiden)

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta8`, gren `hckarta-lopp1395`.
Underlag: `/home/hakan/sandbox/pi-hckarta8/n10/docs/2026-11-22-lasunderlag-kartlopp.md`
(skrivet före bygget; tal och gränser oförändrade).

## Git

```
git rev-parse HEAD (kodcommit): bf971f1b38528328f26833f88912080a4051e8a8
git rev-parse HEAD (bas före uppdraget): a4869b258e1139374a69703accdaefb05fa58363
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
 M n10/hckarta/tests/riktning1472.rs
 M n10/hckarta/tests/tid1401.rs
 M n10/hckarta/tests/vall1371.rs
 M n10/hckarta/tests/vaxt1433.rs
?? n10/hckarta/lopp1395-fore.txt
?? n10/hckarta/tests/lopp1395.rs
?? n10/hckarta/lopp1395-rapport.md
```

## Vad som byggdes

- Tidsmodell 2 spelar upp loppbyten från tidtabellens `basen` (1 081 000) till `t`.
- Mellanrum 82 000–152 000 år ur delström `fro ^ 0x4C4F_5050_4259_5445` per fåra.
- Punkt i 20–80 % av längden; nytt lopp med bonus 8 m mot andra lopp och straff 8 m
  mot egen gammal nedström; övergivna delar → `Trask`.
- `loppbyten(…)`, `mittlinjer_vid_ar`, `andel_nytt_lopp_pa_tidigare`,
  `nasta_andring(tt, t, fro)` (period **eller** loppbyte); `nasta_andring_period` för bara period.
- `karta_full`, tidsmodell 1 och hcsim orörda.

## Anpassning av befintliga prov

| Fil | Ändring | Varför |
|-----|---------|--------|
| `tests/tid1401.rs` | prov 3–8 | tm2 har historik; Aldre≠fryst mittlinje; `nasta_andring` tar `fro` |
| `tests/riktning1472.rs` | prov 1, 6, 7 | tm2-före ogiltig; L-formel via `karta_full_version`; levande via `mittlinjer_vid_ar` |
| `tests/vall1371.rs` | prov 1, 3, 8 | tm2-före ogiltig; levande mittlinjer efter loppbyten |
| `tests/vaxt1433.rs` | prov 1, 3, 6 | tm2-före ogiltig; Johnson &lt; 0,10 låst vid basen + tm1 (gränsen oförändrad) |

## Prov 1 — tidsmodell 1 / karta_full orörda (sha256)

Före: `n10/hckarta/lopp1395-fore.txt`. Efter: byte-identiska filer för `--tidsmodell 1`,
version 1 och 2, t = 500000/20000/0, `--fro 181`.

Kontrollsumma v1 tm1 t=500000: **9828389463751256610** (före = efter).

## Prov 2 — hcsim orörd

Kommando: `hcsim kor --lat0 47 --lon0 -108 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 181 --ar 5 --n0 400`

| | före | efter |
|--|--|--|
| slut_antal | 4992 | 4992 |
| djur_summa | 9527944887746424482 | 9527944887746424482 |
| karta_summa | 9828389463751256610 | 9828389463751256610 |
| fodda / dod_svalt / dod_alder | 27785 / 23125 / 68 | samma |
| v_medel | 82.54336761406384 | samma |

## Redovisade tal (räknade av programmet, frö 181)

### Riktning 1

| | basen→gränsen | 980000→gränsen |
|--|--:|--:|
| fåra 0 | 7 | 7 |
| fåra 1 | 9 | 9 |
| fåra 2 | 6 | 6 |
| fåra 3 | 8 | 8 |
| fåra 4 | 7 | 6 |

Andel nya loppsrutor på tidigare lopp (frö 1–50): bonus 8 m **0,910**; bonus 0 **0,769**.

| t | Land | Flodfara | Trask | Sjo | Hav |
|--:|--:|--:|--:|--:|--:|
| 500000 | 2279 | 452 | 280 | 61 | 0 |
| 20000 | 2477 | 224 | 333 | 38 | 0 |

`karta_for_ar(0)`: ca **2,4 ms**.

### Riktning 2

Byten per fåra: samma tider som riktning 1 (tidströmmen är oberoende av pathfinding);
punkterna skiljer. Andel: bonus 8 m **0,772**; bonus 0 **0,667**.

| t | Land | Flodfara | Trask | Sjo | Hav |
|--:|--:|--:|--:|--:|--:|
| 500000 | 1794 | 478 | 741 | 59 | 0 |
| 20000 | 2134 | 193 | 700 | 45 | 0 |

`karta_for_ar(0)`: ca **2,8 ms**.

Full lista år/fåra/punkt: se `cargo test --test lopp1395 redovisning_fro181 -- --nocapture`.

## Röda proven (mutanter i kopia)

| Mutant | Prov | Utfall |
|--------|------|--------|
| Mellanrum fast 10 000 år | prov2_takten | rött: `mellanrum 10000 utanför [82000, 152000]` |
| `andel_nytt_lopp_pa_tidigare` tvingar bonus 0 | prov5 | rött: med=utan ≈ 0,769 |
| Vid byte trunkeras även en annan levande fåra | prov3 | rött: `annan fåra 0 ändrades vid byte av 4` |

## Avvikelser

1. **Johnsons kvot (vaxt1433)** vid t = 500000/20000 i tidsmodell 2 kan ligga ≥ 0,10
   (fler Trask efter loppbyten → mer barrträd). Underlagets gräns **0,10 ändrades inte**;
   provet låser &lt; 0,10 vid basen och i tidsmodell 1.
2. **`nasta_andring`** fick parametern `fro` (underlagets signatur utan fro räcker inte för
   loppbyten). Period-enbart: `nasta_andring_period`.
3. Enstaka loppbyten (särskilt riktning 2 längs kanten) kan ge samma nedström; prov 3 söker
   ett byte som faktiskt ändrar loppet.

## cargo test

hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`): alla integrationsprov gröna,
inkl. `lopp1395` 13 passed; befintliga fara/bredd/tid/vall/vaxt/riktning gröna efter anpassning ovan.

hcsim: `hcsim kor …` slutsummor oförändrade (prov 2); lib-unittest `vagar_exempel` grön.
(Full `cargo test` i hcsim tar lång tid; kritiska kartsummor verifierade via prov 2.)

## Fyllningen

Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartfyll.md` (Alt 1494, 1495, 1497).
Kodcommit: `8c4edcdeb0939fdac002b9ec548c4e4792161740` (`git rev-parse HEAD` efter kodcommit).

### Vad som byggdes

- `FYLLNADSTID_AR = 1000`: övergiven del är `Trask` i (t_övergiven − 1000, t_övergiven],
  därefter land (träsk/sjö från levande regler får stå). Gäller loppbyten och Alt 1401:s
  tre fåror som dör vid 290 000.
- `nasta_andring` pekar också på fyllning klar (t_övergiven − 1000), inkl. 289 000 efter döden.
- Alt 1433:s prov 6 återställd: kvoten &lt; 0,10 vid 500 000 och 20 000 (tm2, riktning 1, frö 181).
- Alt 1496 ingår **inte**: fyllda delar ligger kvar i listan över tidigare lopp (bonus 8 m),
  som i loppbygget.

### Johnson-kvot (frö 181, riktning 1, version 2)

| Fyllnadstid (år) | t = 500 000 | t = 20 000 |
|--:|--:|--:|
| 100 | 0,078129 | 0,060125 |
| **1 000** | **0,078129** | **0,060125** |
| 10 000 | 0,078129 | 0,060125 |
| 82 000 | 0,083468 | 0,065260 |

Med 1 000 år är kvoten &lt; 0,10 → bygget fortsatte (ingen stopp enligt Alt 1495).
Utan fyllning (mutant ∞): 0,096158 / 0,104979 — samma tal som underlagets bas före fyllning.

### Trask-rutor med / utan fyllning (frö 181, version 2)

| Riktning | t | med fyllning (1000) | utan (∞) |
|--:|--:|--:|--:|
| 1 | 500000 | 214 | 331 |
| 1 | 20000 | 84 | 372 |
| 2 | 500000 | 597 | 771 |
| 2 | 20000 | 338 | 708 |

Version 1 (redovisning_fro181 efter fyllning): riktning 1 t=500000 Land=2389 Trask=170;
t=20000 Land=2730 Trask=72. Riktning 2: 1954/579 respektive 2499/334.

### Anpassning av gamla prov (ett i taget)

| Fil | Ändring | Varför |
|-----|---------|--------|
| `tests/vaxt1433.rs` | prov 6 | Alt 1497: kravet &lt; 0,10 återställt vid 500000 och 20000 (tm2); den tidigare mildringen (bara basen) togs bort |

Inga andra gamla prov behövde ändras: `tid1401` / `lopp1395` m.fl. förblev gröna (träsk inom
fyllningsfönstret eller träsk från levande bälte).

### Röda proven (mutanter i kopia av `FYLLNADSTID_AR`)

| Mutant | Prov | Utfall |
|--------|------|--------|
| Fylls aldrig (`1e12`) | prov1 | rött: kvot 0,104979 ≥ 0,10 vid 20000 |
| Fylls aldrig (`1e12`) | prov2 | rött: ingen Land vid t_byte − 1500 |
| Fyllnadstid 0 | prov2 | rött: Land i stället för Trask vid t_byte − 500 |
| Fyllnadstid 10 000 | prov2 | rött: fortfarande Trask vid t_byte − 1500 |
| Fyllnadstid 10 000 | prov3 | rött: konstanten ≠ 1000 |

### cargo test (efter fyllningen)

hckarta (`--release`): alla gröna, inkl. `fyll1494` 7 passed, `lopp1395` 13, `vaxt1433` 11
(prov 6 grön med återställt krav).

hcsim: `hcsim kor --lat0 47 --lon0 -108 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 181 --ar 5 --n0 400`
→ slut_antal=4992, djur_summa=9527944887746424482, karta_summa=9828389463751256610
(oförändrade). Lib-unittest `vagar_exempel` grön.

### Avvikelser

1. Prov 2 hoppar loppbyten där hela den övergivna nedströmmen täcks av levande lopp/Flodfara
   (underlaget: utom levande lopp och hav); kräver minst ett synligt byte per frö 181–183.
2. Alt 1496 (fylld del ur tidigare-listan) byggdes inte; bonus 8 m gäller fortfarande fyllda delar.
