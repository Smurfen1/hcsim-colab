# Rapport: Alt 1604 (stigningsgräns) och Alt 1605 (lopp som möts)

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta11`, gren `hckarta-uppfor1604`.
Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartstd3.md` (skrivet före bygget).
Mätning som visade felet: `/home/hakan/Projekt/software30/piharness/docs/evolution/kartstd-diag.txt`
(riktning 2: alla djupa rutor förklarade av stigning; riktning 1: ytan ofta från korsning).

## Git

```
git rev-parse HEAD (bas före uppdraget): 6786825863d3f39d5e6ffe00930348a4b2de8b5a
git rev-parse HEAD (efter steg 1 / Alt 1604): 9ed6a9801c899df594bd2c6305b42c4d3624f13d
git rev-parse HEAD (efter steg 2 / Alt 1605): a5bedc4097f399fc644cd255e10b52123a0de7d8
git rev-parse HEAD (lutning-2-växel, del av steg 2): 1e609fc65666589b5d368636db8b56ba198e78ca
```

`git status --short` före första commit: tom (ren arbetskatalog på `hckarta-uppfor1604`).

## Vad som byggdes

### Steg 1 (Alt 1604)

- `LOPP_MAX_STIGNING_M` = 5 m (eget val).
- Efter loppbyte: om kostnadssteget bryter gränsen väljs bland tillåtna steg det som stiger minst; finns inget avbryts bytet.
- Gäller vid **lutning 2** i tidsmodell 2. Lutning 1, första loppen och tidsmodell 1 orörda.
- Provhjälp: `prova_nytt_lopp`, `mat_linjer_tm2` / `mat_linjer_tm2_prov`, `LoppProvMutant`.

### Steg 2 (Alt 1605)

- Nytt lopp efter loppbyte som når ett **annat levande** lopp slutar där.
- Fallande yta i TM2+lutning 2: per lopp, sedan min i delade rutor — lägre yta propagerar inte som `prev` in i annat lopp.
- Tidsmodell 1 behåller gammal itererad delning.
- Mutant `MotetAvslaget`: ingen mötesstopp + gammal yta (rött i korsmåttet).

## Prov 1 rött på dagens kod

Handbyggd rygg 10 m, plan 0,30 m/km mot 135°, bonus bakom ryggen, riktning 2:

| Körning | max stigning | Utfall mot 5 m-krav |
|---------|-------------:|---------------------|
| utan gräns (`max_stigning` = 1e9, dagens beteende) | 9,576 m | rött |
| Alt 1604 (5 m) | bytet skedde inte | grönt |
| mutant 10 m | 9,576 m | rött (prov 5) |

## Provutfall

| Prov | Utfall | Anmärkning |
|------|--------|------------|
| 1 ryggen | grönt | rött utan gräns; grönt med 5 m |
| 2 frön steg 1 | grönt | max stigning ≤ 5 m; riktning 2: 0 over7 |
| 3 frön steg 2 | grönt | riktning 1: 0 kors (`\|djup−2−stigning\| > 0,01`) |
| 4 TM1 / hcsim | grönt | hcsim slut_antal 4992, djur_summa oförändrad |
| 5 mutanter | grönt | 10 m röd i rygg; MotetAvslaget kors=143, Ingen=0 |

## Gate efter steg 1

Riktning 2, over7-summa frö 1–20: **37 / 33 / 33** (Aldre/Ovre/Gransen) före → **0 / 0 / 0** efter steg 1.
Steg 2 genomfördes.

## Tabell per frö (före = MaxStigning 1e9; e1 = efter Alt 1604; e2 = efter Alt 1605)

Byten-antalet oförändrat (omdirigering inom gränsen, sällan avbrott på lutning-2-planet). Fullständiga tabeller:

### Aldre t=500000, riktning 2

| frö | byten f/e1/e2 | over7 f/e1/e2 | p95 f/e1/e2 | max f/e1/e2 | träsk e1/e2 | Johnson e1/e2 |
|----:|--------------:|--------------:|------------:|------------:|------------:|--------------:|
| 1 | 23/23/23 | 0/0/0 | 2.618/2.618/3.502 | 4.445/4.445/4.554 | 0.1172/0.1518 | 0.0995/0.1161 |
| 2 | 25/25/25 | 0/0/0 | 4.053/4.053/4.367 | 5.747/5.747/6.287 | 0.1451/0.1392 | 0.1124/0.1098 |
| 3 | 21/21/21 | 0/0/0 | 3.255/3.255/2.627 | 5.761/5.761/3.877 | 0.1295/0.1312 | 0.1049/0.1062 |
| 4 | 23/23/23 | 7/0/0 | 6.230/4.791/3.854 | 8.132/6.438/5.385 | 0.1159/0.1421 | 0.0984/0.1113 |
| 5 | 22/22/22 | 0/0/0 | 3.232/3.252/3.562 | 4.560/5.725/4.466 | 0.1448/0.1491 | 0.1125/0.1153 |
| 6 | 22/22/22 | 0/0/0 | 3.293/3.293/3.232 | 5.031/5.031/5.031 | 0.1262/0.1485 | 0.1034/0.1143 |
| 7 | 22/22/22 | 0/0/0 | 2.842/2.842/3.382 | 3.849/3.849/5.454 | 0.1368/0.1418 | 0.1085/0.1110 |
| 8 | 24/24/24 | 0/0/0 | 3.206/3.206/4.436 | 4.460/4.460/6.291 | 0.1486/0.1529 | 0.1141/0.1166 |
| 9 | 22/22/22 | 0/0/0 | 3.255/3.255/2.710 | 5.454/5.454/3.921 | 0.1395/0.1352 | 0.1097/0.1082 |
| 10 | 22/22/22 | 0/0/0 | 2.625/2.625/2.626 | 3.361/3.361/4.576 | 0.1332/0.1192 | 0.1067/0.1003 |
| 11 | 22/22/22 | 0/0/0 | 4.465/4.465/4.529 | 6.755/6.755/5.803 | 0.1292/0.1309 | 0.1048/0.1062 |
| 12 | 22/22/22 | 0/0/0 | 3.349/3.349/3.288 | 5.765/5.765/4.523 | 0.1358/0.1591 | 0.1080/0.1196 |
| 13 | 23/23/23 | 0/0/0 | 3.179/3.179/3.744 | 5.179/5.179/5.179 | 0.0960/0.1196 | 0.0894/0.1006 |
| 14 | 24/24/24 | 0/0/0 | 3.482/3.482/2.625 | 5.200/5.200/3.254 | 0.1309/0.1196 | 0.1056/0.1009 |
| 15 | 23/23/23 | 0/0/0 | 3.835/3.835/3.249 | 6.429/6.429/4.348 | 0.1681/0.1677 | 0.1234/0.1236 |
| 16 | 22/22/22 | 5/0/0 | 5.584/5.541/4.018 | 8.134/6.898/6.780 | 0.1624/0.1518 | 0.1209/0.1157 |
| 17 | 21/21/21 | 0/0/0 | 3.175/3.175/3.478 | 5.213/5.213/5.175 | 0.1302/0.1378 | 0.1053/0.1088 |
| 18 | 22/22/22 | 2/0/0 | 3.250/3.145/4.079 | 7.767/4.549/5.355 | 0.1249/0.1395 | 0.1029/0.1098 |
| 19 | 23/23/23 | 19/0/0 | 8.237/3.797/4.931 | 12.018/6.699/6.838 | 0.1568/0.1259 | 0.1180/0.1033 |
| 20 | 22/22/22 | 4/0/0 | 4.781/4.017/4.771 | 7.986/6.435/6.668 | 0.1043/0.1368 | 0.0931/0.1085 |

### Övriga tider — over7-summa frö 1–20

| fas | t | r1 | r2 |
|-----|---|---:|---:|
| före | 500000 | 13 | 37 |
| före | 20000 | 0 | 33 |
| före | 0 | 0 | 33 |
| efter1 | 500000 | 12 | 0 |
| efter1 | 20000 | 0 | 0 |
| efter1 | 0 | 0 | 0 |
| efter2 | 500000 | 0 | 0 |
| efter2 | 20000 | 0 | 0 |
| efter2 | 0 | 0 | 0 |

Riktning 1 kors-summa (fårrutor med `|djup−2−stigning| > 0,01`): före/e1 ≈ 111–112 (Aldre) → efter2 **0**.

Rådata: kör `UPPFOR_FAS=fore|efter1|efter2 cargo test --release --test uppfor1604 matning_rapport_tsv -- --ignored --nocapture`.

## cargo test

### hckarta

```
uppfor1604: 5 passed; 0 failed; 1 ignored
lopp1395: 13 passed
övriga utom lutning1485: gröna
lutning1485: 14 passed; 1 failed (prov6_johnsons_kvot)
```

`prov6_johnsons_kvot` (frö 181, r2+l2, Aldre): Johnson **0,104179** (gräns &lt; 0,10; före stegen 0,095934).
Orsak: mötesstoppet (Alt 1605) kortar nya lopp och ökar träsk (381 mot 332); stigningsgränsen ensam (MotetAvslaget) behåller 0,095934.
Provfilen `lutning1485.rs` får inte ändras i detta uppdrag. Underlaget skjuter träsk/Johnson till Alt 1608/1609.

### hcsim

```
14 passed; 0 failed
slut_antal 4992, djur_summa 9527944887746424482 (oförändrade)
```

## Röda prov (avsiktliga)

1. Rygg utan gräns: stigning 9,576 m &gt; 5 m.
2. Mutant 10 m: samma, rött mot 5 m-kravet.
3. Mutant MotetAvslaget: kors_avvik 143 &gt; 0 (rött mot prov 3).
4. Befintligt lås: `lutning1485::prov6_johnsons_kvot` rött efter steg 2 (se ovan).

## Avvikelser

1. **Johnson frö 181** efter Alt 1605 över 0,10 (se cargo test). Inget tal i Alt 1604/1605 ändrat; träskregeln orörd.
2. Alt 1604/1605 aktiveras bara vid **lutning ≥ 2** så att lutning 1 / `loppbyten` / befintliga lopp1395-prov förblir byte-kompatibla.
3. Byten-antal per frö oförändrat på lutning-2-planet (vägarna omdirigeras inom 5 m i stället för att avbrytas).
4. Referensfiler `*-fore.txt` orörda (inga tidsmodell-2-hashar att uppdatera under detta uppdrag).

## LASMIG

Avsnitt 5b (Alt 1604) och 5c (Alt 1605) tillagda under fåror som objekt.

## Alt 1605 avslagen

Skriven 2026-11-22 (eftermiddag) av Cursor i samma arbetsträd/gren efter att
`prov6_johnsons_kvot` blivit rött med mötet i produktionsvägen.

```
git rev-parse HEAD (före avslag / efter steg 2-rapport): eca5bb76622722fe7238caa7f774755d3f9e6642
git rev-parse HEAD (kod-commit Alt 1605 avslagen):       52ecc956c6e5d4d2736e18a9836c814e509dc338
```

`git status --short` före kod-commit: `M` på `flod.rs`, `uppfor1604.rs`, `LASMIG.md`.

### Vad som ändrades

- `LoppProvMutant::Ingen` (produktion) = Alt 1604 på, Alt 1605 av (gammal delad yta, inget mötesstopp).
- Ny mutant `LoppProvMutant::MedMote` = Alt 1605 på (mötesstopp + mötesyta); bara proven.
- `MotetAvslaget` oförändrad som uttrycklig «möte av»-mutant (samma utfall som produktion).
- Prov 3 körs med `MedMote`; nytt prov 6: produktion = `MotetAvslaget` för frö 1–20 (120 kartor).
- LASMIG 5c: «byggd, avslagen i produktionen, väntar på omröstning».

### Johnsons kvot frö 181 (r2+l2, produktionsväg)

| Period | t | Johnson | Gräns |
|--------|--:|--------:|------:|
| Aldre | 500 000 | **0,095934** | &lt; 0,10 |
| Ovre | 20 000 | **0,078597** | &lt; 0,10 |

Före avslaget (med möte i produktion): Aldre 0,104179 (rött). Med enbart Alt 1604: samma 0,095934 som före steg 2.

### cargo test --release

#### hckarta (alla gröna)

| Provsvit | Antal |
|----------|------:|
| filer / lib (tomma) | 0 + 0 |
| flod1343 | 11 |
| fyll1494 | 10 |
| kartan | 7 |
| kontroll | 1 |
| lopp1395 | 13 |
| lutning1485 | **15** (prov6_johnsons_kvot grönt igen) |
| paleokarta | 3 |
| riktning1472 | 9 |
| smoke_s3 | 5 |
| tid1401 | 6 |
| tidtabell | 5 |
| tsv | 8 |
| vall1371 | 12 |
| **uppfor1604** | **6 passed; 1 ignored** |
| vaxt1433 | 11 |
| vatten1362 | 11 |

#### hcsim (alla gröna)

| Provsvit | Antal |
|----------|------:|
| lib / diagnos / hjarnvakt / kartvy | 1 + 1 + 6 + 8 |
| hjsignal1423 | 13 |
| kopier1462 | 13 |
| prover | 54 |
| spara1302 | 13 passed; 2 ignored |
| utforsk1463 | 14 |

`slut_antal` 4992, `djur_summa` 9527944887746424482 (oförändrade i prov 4).

### Avvikelser

1. Alt 1605-koden står kvar men nås bara via `MedMote`; produktion och `Ingen` har samma beteende som tidigare `MotetAvslaget` för möte/yta.
2. Träsk/Johnson-regeln orörd; Alt 1605 tas upp igen tillsammans med Alt 1608/1609.
3. Inga befintliga provfiler utanför `uppfor1604.rs` ändrade.
