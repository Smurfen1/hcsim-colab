# Rapport: träskbälte 2 och möte i produktionen (Alt 1643, 1644)

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta12`, gren `hckarta-trask1643`.
Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-johntrask.md` (skrivet före bygget).
Föregående pass avbröts (felkod 144) under steg 1; osparade ändringar behölls och arbetet återupptogs.

## Git

```
git status --short före (återupptag): M LASMIG.md, M flod.rs, ?? tests/trask1643.rs
git rev-parse HEAD (bas):            53d4b702e09fc61ab398496c1a9e14008f639e04
git rev-parse HEAD (efter steg 1):   55445a7351826f868a06d74562698d7aab6c2690
git rev-parse HEAD (efter steg 2):   4e4d39bad65a021f84e28d9e940cfc7995026c56
git rev-parse HEAD (efter provet):   1447a34be0f43c1cfcd98d80cec359c71b440152
```

## Steg 0 (Fables mätning)

Träsk per Chebyshev-avstånd 0–3, med/utan möte, fröna 1–20, Aldre (t=500000) och Ovre (t=20000).
Olagliga = bältesträsk med höjd ≥ närmaste mittlinjeyta + 1 m. **0 olagliga** med mötet på → ingen stopp.

| avst | möte | t | träsk | land | andel | olagliga | d0 | d1 | d2 | d3 | över3 |
|-----:|:----:|--:|------:|-----:|------:|---------:|---:|---:|---:|---:|------:|
| 0 | av | 500000 | 51 | 60173 | 0,0008 | 0 | 0 | 21 | 11 | 5 | 14 |
| 0 | av | 20000 | 0 | 60520 | 0,0000 | 0 | 0 | 0 | 0 | 0 | 0 |
| 0 | på | 500000 | 63 | 60162 | 0,0010 | 0 | 0 | 18 | 14 | 9 | 22 |
| 0 | på | 20000 | 0 | 60520 | 0,0000 | 0 | 0 | 0 | 0 | 0 | 0 |
| 1 | av | 500000 | 1720 | 58504 | 0,0286 | 0 | 0 | 1690 | 11 | 5 | 14 |
| 1 | av | 20000 | 686 | 59834 | 0,0113 | 0 | 0 | 686 | 0 | 0 | 0 |
| 1 | på | 500000 | 1768 | 58457 | 0,0294 | 0 | 0 | 1723 | 14 | 9 | 22 |
| 1 | på | 20000 | 680 | 59840 | 0,0112 | 0 | 0 | 680 | 0 | 0 | 0 |
| 2 | av | 500000 | 4853 | 55371 | 0,0806 | 0 | 0 | 1690 | 3144 | 5 | 14 |
| 2 | av | 20000 | 2113 | 58407 | 0,0349 | 0 | 0 | 686 | 1427 | 0 | 0 |
| 2 | på | 500000 | 5056 | 55169 | 0,0840 | 0 | 0 | 1723 | 3302 | 9 | 22 |
| 2 | på | 20000 | 2111 | 58409 | 0,0349 | 0 | 0 | 680 | 1431 | 0 | 0 |
| 3 | av | 500000 | 8056 | 52168 | 0,1338 | 0 | 0 | 1690 | 3144 | 3208 | 14 |
| 3 | av | 20000 | 3597 | 56923 | 0,0594 | 0 | 0 | 686 | 1427 | 1484 | 0 |
| 3 | på | 500000 | 8430 | 51795 | 0,1400 | 0 | 0 | 1723 | 3302 | 3383 | 22 |
| 3 | på | 20000 | 3664 | 56856 | 0,0605 | 0 | 0 | 680 | 1431 | 1553 | 0 |

## Prov rött på dagens kod (före steg 1)

`prov1_johnsons_kvot_fro_1_20` (produktion, avstånd 3, möte av):

```
prov1 t=500000 fro=1 johnson=0.099478 trask=353 andel=0.1172
prov1 t=500000 fro=2 johnson=0.112385 trask=437 andel=0.1451
Johnsons kvot 0.112385… ska vara < 0,10 (fro=2 t=500000)
test result: FAILED. 0 passed; 1 failed
```

## Vad som byggdes

### Steg 1 (Alt 1643)

- I `lagg_foror_som_objekt`: vid lutning ≥ 2 sätts `trask_avstand = 2` i Aldre och Ovre; Gransen behåller 3.
- `flodregler`, tidsmodell 1, lutning 1 och `karta_full` orörda.
- Provmutant `TraskProvMutant::Avstand(n)` för steg 0 och röda mutanter.
- LASMIG: träskbälte 2 = eget val med underlagets USGS-citat.

### Steg 2 (Alt 1605 i produktionen)

- `LoppProvMutant::Ingen` = möte på; `MotetAvslaget` = mutant (möte av); `MedMote` alias för på.
- LASMIG 5c uppdaterad.
- `uppfor1604.rs` prov 6: produktionen har mötet (jämför mot `MedMote`). Ändrade rader:

| Rad (efter) | Före | Efter | Skäl |
|------------:|------|-------|------|
| 1 | «bara via MedMote» | «produktion efter Alt 1643» | modulkommentar |
| 493 | «har inte mötet» / `MotetAvslaget` | «har mötet» / `MedMote` | doksträng |
| 495 | `prov6_produktion_utan_mote` | `prov6_produktion_har_mote` | namn speglar kravet |
| 507 | `let (av, _)` | `let (pa, _)` | variabelnamn |
| 517 | `MotetAvslaget` | `MedMote` | jämförelsemål = möte på |
| 519 | `expect("MotetAvslaget")` | `expect("MedMote")` | samma |
| 521, 525, 529 | `av.…` | `pa.…` | samma |
| 537 | «produktion = MotetAvslaget» | «produktion = MedMote» | loggrad |

### Steg 3 (Alt 1644)

- `tests/trask1643.rs`: Johnson &lt; 0,10 på fröna 1–20 (Aldre+Ovre); mutant avstånd 3 ≥ 16/20 röda i Aldre; `MotetAvslaget` korsavvik &gt; 0.
- Ignore-prov: steg 0-mätning och Johnson-rapportmätning.

## Johnson och träskandel per frö

före = Avstand(3)+MotetAvslaget; e1 = Ingen+MotetAvslaget (bälte 2, möte av); e2 = Ingen+MedMote (bälte 2, möte på = produktion efter steg 2).

### Aldre t = 500 000

| frö | J före | andel före | J e1 | andel e1 | J e2 | andel e2 |
|----:|-------:|-----------:|-----:|---------:|-----:|---------:|
| 1 | 0,099478 | 0,1172 | 0,077272 | 0,0694 | 0,086522 | 0,0893 |
| 2 | 0,112545 | 0,1455 | 0,086426 | 0,0900 | 0,083760 | 0,0837 |
| 3 | 0,105187 | 0,1302 | 0,083020 | 0,0827 | 0,081904 | 0,0794 |
| 4 | 0,098450 | 0,1159 | 0,075457 | 0,0661 | 0,082249 | 0,0804 |
| 5 | 0,112357 | 0,1445 | 0,085111 | 0,0867 | 0,085029 | 0,0854 |
| 6 | 0,103576 | 0,1265 | 0,079012 | 0,0737 | 0,084516 | 0,0854 |
| 7 | 0,108854 | 0,1375 | 0,083160 | 0,0827 | 0,082755 | 0,0817 |
| 8 | 0,114433 | 0,1493 | 0,087794 | 0,0929 | 0,088238 | 0,0932 |
| 9 | 0,111820 | 0,1438 | 0,082354 | 0,0810 | 0,082100 | 0,0797 |
| 10 | 0,107140 | 0,1342 | 0,083064 | 0,0827 | 0,079305 | 0,0741 |
| 11 | 0,103678 | 0,1269 | 0,080183 | 0,0764 | 0,079615 | 0,0741 |
| 12 | 0,108590 | 0,1372 | 0,082776 | 0,0820 | 0,087824 | 0,0923 |
| 13 | 0,089080 | 0,0953 | 0,071743 | 0,0575 | 0,075035 | 0,0644 |
| 14 | 0,105774 | 0,1312 | 0,080824 | 0,0777 | 0,078557 | 0,0717 |
| 15 | 0,123737 | 0,1687 | 0,090211 | 0,0983 | 0,091368 | 0,1003 |
| 16 | 0,121032 | 0,1627 | 0,087131 | 0,0913 | 0,087131 | 0,0913 |
| 17 | 0,104961 | 0,1295 | 0,082024 | 0,0804 | 0,085462 | 0,0880 |
| 18 | 0,102573 | 0,1242 | 0,079997 | 0,0757 | 0,083753 | 0,0840 |
| 19 | 0,117983 | 0,1568 | 0,087336 | 0,0920 | 0,080546 | 0,0771 |
| 20 | 0,092914 | 0,1040 | 0,072950 | 0,0604 | 0,082193 | 0,0807 |
| 181 | 0,096090 | 0,1106 | 0,076314 | 0,0678 | 0,078549 | 0,0717 |

Aldre fröna 1–20 med J ≥ 0,10: **16 före**, **0 efter e1**, **0 efter e2**. Max e2 = 0,091368 (frö 15).

### Ovre t = 20 000

| frö | J före | andel före | J e1 | andel e1 | J e2 | andel e2 |
|----:|-------:|-----------:|-----:|---------:|-----:|---------:|
| 1 | 0,072737 | 0,0552 | 0,062386 | 0,0327 | 0,061028 | 0,0297 |
| 2 | 0,078132 | 0,0668 | 0,065109 | 0,0387 | 0,068606 | 0,0463 |
| 3 | 0,078442 | 0,0674 | 0,064654 | 0,0377 | 0,061330 | 0,0304 |
| 4 | 0,067996 | 0,0449 | 0,060426 | 0,0284 | 0,060125 | 0,0278 |
| 5 | 0,069980 | 0,0492 | 0,060125 | 0,0278 | 0,061631 | 0,0311 |
| 6 | 0,078287 | 0,0671 | 0,063746 | 0,0357 | 0,063443 | 0,0350 |
| 7 | 0,080615 | 0,0720 | 0,065716 | 0,0400 | 0,063594 | 0,0354 |
| 8 | 0,075043 | 0,0601 | 0,062990 | 0,0340 | 0,063443 | 0,0350 |
| 9 | 0,063897 | 0,0360 | 0,056078 | 0,0188 | 0,061631 | 0,0311 |
| 10 | 0,076740 | 0,0638 | 0,065867 | 0,0403 | 0,063897 | 0,0360 |
| 11 | 0,070133 | 0,0496 | 0,058773 | 0,0248 | 0,058773 | 0,0248 |
| 12 | 0,080926 | 0,0727 | 0,067235 | 0,0433 | 0,067540 | 0,0440 |
| 13 | 0,077050 | 0,0644 | 0,063897 | 0,0360 | 0,060276 | 0,0281 |
| 14 | 0,089223 | 0,0902 | 0,068759 | 0,0466 | 0,066931 | 0,0426 |
| 15 | 0,074889 | 0,0598 | 0,063141 | 0,0344 | 0,060878 | 0,0294 |
| 16 | 0,070897 | 0,0512 | 0,059073 | 0,0254 | 0,059524 | 0,0264 |
| 17 | 0,067996 | 0,0449 | 0,060426 | 0,0284 | 0,064048 | 0,0364 |
| 18 | 0,081081 | 0,0730 | 0,066779 | 0,0423 | 0,064654 | 0,0377 |
| 19 | 0,067692 | 0,0443 | 0,059073 | 0,0254 | 0,059374 | 0,0261 |
| 20 | 0,077823 | 0,0661 | 0,063292 | 0,0347 | 0,067692 | 0,0443 |
| 181 | 0,079062 | 0,0687 | 0,064351 | 0,0370 | 0,065412 | 0,0393 |

## cargo test

### hckarta (`cargo test -j 4 --release --no-fail-fast`)

| Provsvit | Utfall |
|----------|--------|
| bredd1386 | 10 passed; **1 failed** (prov2_hcsim_orord) |
| fara1343 | 9 passed; **1 failed** (prov2_hcsim_orord) |
| fyll1494 | 7 passed |
| granser | 1 passed |
| lopp1395 | 12 passed; **1 failed** (prov2_hcsim_orort) |
| lutning1485 | 14 passed; **1 failed** (prov1b_hcsim_orort); inkl. prov5a och prov6 gröna |
| mutant1584 | 3 passed |
| riktning1472 | 8 passed; **1 failed** (prov2_hcsim_orort) |
| s1–s4 | 5+6+5+8 passed |
| tid1401 | 11 passed; **1 failed** (prov2_hcsim_orort) |
| **trask1643** | **3 passed; 2 ignored** |
| uppfor1604 | 5 passed; **1 failed** (prov4 hcsim); 1 ignored |
| vall1371 | 10 passed; **1 failed** (prov2_hcsim_orort) |
| vaxt1433 | 10 passed; **1 failed** (prov2_hcsim_orort) |

Alla nio röda är samma sorts assert: hcsim `slut_antal`/`djur_summa` mot `*-fore.txt` (väntat 4992 / 9527…; fick 5548 / 4225…). **`karta_summa` oförändrad:** 9828389463751256610.

### hcsim (`cargo test -j 4 --release --no-fail-fast`)

Alla gröna. Antal per mål (ur `test result:`):

| Mål | Utfall |
|-----|--------|
| lib / bin-små | 1+1+6+20 passed |
| flodkant1624 | 3 passed |
| hjsignal1423 | 13 passed |
| kopier1462 | 13 passed |
| prover | 54 passed |
| spara1302 | 13 passed; 2 ignored |
| utforsk1463 | 14 passed |

`hcsim kor` (fro 181, 5 år): `karta_summa` 9828389463751256610 (oförändrad); `slut_antal` 5548 (se avvikelser).

## Röda prov (avsiktliga i trask1643)

1. Mutant avstånd 3: **19 av 20** frön ≥ 0,10 i Aldre (krav ≥ 16).
2. `MotetAvslaget`: korsavvik-summa **143** &gt; 0.
3. Före steg 1: prov1 rött (se ovan).

## Avvikelser

1. **hcsim-slutsummor för djur** skiljer sig från `*-fore.txt` (4992 → 5548) medan **`karta_summa` är densamma**. Orsak: befintlig Alt 1624 i denna träds `hcsim` (`landandel_av` ger Flodfara land &gt; 0 via `fara_bredd_m`), inte Alt 1643/1605. Samma 5548 fås i `sandbox/pi` utan våra ändringar; `pi-hckarta11`:s äldre binär ger fortfarande 4992. Inga tal mildrade; förbjudna filer orörda; **stopp** enligt uppdraget efter redovisning.
2. Inga `*-fore.txt` uppdaterade: tidsmodell-1-hashar och övriga TM2-lutning-1-vägar oförändrade; inga TM2+lutning≥2-hashrader behövde räknas om för gröna proven.
3. `lutning1485::prov6_johnsons_kvot` (frö 181) grönt efter steg 2: Johnson 0,078549 / 0,065412 (&lt; 0,10).
4. Föregående pass avbröts under steg 1; steg 0-utdata och röd-prov-logg fanns kvar i `/tmp` och återanvändes.
