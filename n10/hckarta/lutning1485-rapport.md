# Rapport: Alt 1485 / 1490 / 1488 (höjdkartans lutning mot sydost och fallande vattenyta)

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta9`, gren `hckarta-lutning1485`.
Underlag: `/home/hakan/sandbox/pi-hckarta9/n10/docs/2026-11-22-lasunderlag-kartlutning.md`
(skrivet före bygget; tal och gränser oförändrade).

## Stopp (underlaget)

**Prov 5 klarade inte talen.** Underlaget: ändra inte talen; redovisa mätvärdena och stoppa.
Prov 6 (Johnson) är grönt. Prov 4 (nedskärning ≤ 6 m) är också rött — samma regel: gränsen 6 m
ändrades inte.

## Git

```
git rev-parse HEAD (kodcommit): 21351f436351bad0ee80533bcce4ae778ba24c5e
git rev-parse HEAD (bas före uppdraget): 42843c4b6e45ccee903254d4e3fb643d11c5bc7a
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
 M n10/hckarta/src/main.rs
?? n10/hckarta/lutning1485-fore.txt
?? n10/hckarta/tests/data/lutning1485-fore-v*-tm*-r*-t*/
?? n10/hckarta/tests/lutning1485.rs
?? n10/hckarta/lutning1485-rapport.md
```

## Vad som byggdes

- `--lutning 2` (standard 1): drar bort fönstrets plan (minsta kvadrat, samma som
  `kartrask-plan.rs`) och lägger in 0,30 m/km mot 135° med bevarad medelhöjd över land;
  hav orörda; land under 0 → `Err`.
- Fallande vattenyta längs mittlinjen: `yta = min(förra, höjd − 2 m)`; vid delade rutor
  itereras tills varje fåra är icke-stigande. Träsk mot ytan + 1 m.
- `karta_full` / utan `--lutning 2` / hcsim: oförändrade (lutning 1).
- API: `karta_full_version` / `karta_for_ar` behåller lutning 1; nya
  `*_lutning`-varianter. Befintliga prov oförändrade (inga anropsändringar).
- LASMIG (Alt 1488): bilinjär interpolation; 0,30 m/km Butler [A]; 135° och
  «fårans lutning används för landet» = **eget val**. Standard byts inte (Alt 1490).

## Prov 1 — utan `--lutning 2` orört (sha256)

Före: `n10/hckarta/lutning1485-fore.txt` (+ kopior under `tests/data/`).
Efter: samma hashar byte för byte för version 1 och 2, tidsmodell 1 och 2,
flodriktning 1 och 2, t = 500000/20000/0, utan `--lutning` och med `--lutning 1`.

Kontrollsummor (v1, tm1, r1) oförändrade: 9828389463751256610 / 7307636836510803732 /
9344846268250959607.

## Prov 1b / 2 — hcsim orörd

Kommando: `hcsim kor --lat0 47 --lon0 -108 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 181 --ar 5 --n0 400`

| | före | efter |
|--|--|--|
| slut_antal | 4992 | 4992 |
| djur_summa | 9527944887746424482 | 9527944887746424482 |
| karta_summa | 9828389463751256610 | 9828389463751256610 |
| fodda / dod_svalt / dod_alder | 27785 / 23125 / 68 | samma |
| v_medel | 82.54336761406384 | samma |

## Redovisade tal (räknade av programmet)

Mätsätt: `mat_plan` = `kartrask-plan.rs`; fall/träsk = `kartrask-matning.rs` (frö 1–20, Aldre, version 2, tidsmodell 2).

### Planet

| | lutning (m/km) | riktning (°) | medelhöjd (m) |
|--|--:|--:|--:|
| före | 0,510 | 270,8 | 179,921 |
| efter lutning 2 | 0,300 | 135,0 | 179,921 |

### Fall start→slut och träsk per mittlinjeruta (frö 1–20)

| | fall (m) | träsk/mitt |
|--|--:|--:|
| lutning 1, riktning 1 | −52,7 | 0,456 |
| lutning 1, riktning 2 | −29,0 | 2,139 |
| lutning 2, riktning 1 | +45,6 | 0,083 |
| lutning 2, riktning 2 | +52,4 | **0,998** |

### Johnsons kvot (frö 181, riktning 2, lutning 2)

| t | kvot | gräns |
|--|--:|--|
| 500 000 (Aldre) | 0,076616 | < 0,10 |
| 20 000 (Ovre) | 0,069827 | < 0,10 |

### Vattentyper frö 181, riktning 2, lutning 2, version 2, tidsmodell 2

| t | Land | Flodfara | Trask | Sjo | Hav |
|--|--:|--:|--:|--:|--:|
| 500 000 | 2805 | 0 | 206 | 61 | 0 |
| 20 000 | 2878 | 0 | 148 | 46 | 0 |

### Riktning (riktning 2 + lutning 2, frö 1–50 Aldre)

medel **132,0°**, min 99,0°, max 168,0°, spann 69,0° (n = 250) — inom 125–140°, spann ≥ 10°.

## Provutfall

| Prov | Utfall | Anmärkning |
|------|--------|------------|
| 1 utan lutning 2 | grönt | sha256 = före |
| 1b hcsim | grönt | slutsummor = före |
| 2 planet | grönt | 0,300 m/km mot 135,0°; medel lika |
| 3 vattenyta | grönt | 0/200 stigande med lutning 2; 200/200 utan |
| 4 nedskärning | **rött** | max **6,274 m** > 6 m (fro=4, r=2, ruta 25,28) |
| 5 träsk | **rött** | träsk/mitt **0,998** ∉ 0,24–0,96; öst/väst **0,405** ∉ 0,5–2,0 |
| 6 Johnson | grönt | 0,077 / 0,070 < 0,10 |
| 7 riktning | grönt | 132,0°; spann 69° |
| 8 land ≥ 0 | grönt | 30 m/km ger fel |
| 9 LASMIG | grönt | bilinjär; eget val på 135°-rad; Butler |

## Röda proven (mutanter i kopia)

| Mutant | Prov | Utfall |
|--------|------|--------|
| Planet mot 315° i stället för 135° | 2 | rött mot 135±0,5° (mätt 315,0°) |
| Vattenyta = höjd − 2 utan min-regeln | 3 | rött: stigande yta på DEM efter plan |
| Lutning 30 m/km | 8 | rött: `Err` landruta under 0 |

## Avvikelser (underlagets tal oförändrade)

1. **Prov 5 — träsk per mittlinjeruta** med riktning 2 + lutning 2: uppmätt **0,998**
   (mål 0,24–0,96, hälften–dubbla riktning 1:s 0,48). Öst/väst-kvot **0,405** (mål 0,5–2,0).
   Bygget stoppas här enligt underlaget; inga tal ändrade.
2. **Prov 4 — nedskärning:** max **6,274 m** > 6 m. Fallande yta + lokal resttopografi ger
   djupare snitt än Butler-taket på enstaka rutor. Gränsen 6 m oförändrad.

## cargo test

hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`):

- `lutning1485`: 9 passed, **2 failed** (prov 4, prov 5); redovisning ok.
- Befintliga (fara/bredd/tid/vall/vaxt/riktning/lopp/fyll/s1–s4/granser): alla gröna
  utan ändring av anrop (API bakåtkompatibel via lutning-1-wrappers).

hcsim (`CARGO_TARGET_DIR=…/n10/hcsim/target`, `--release`): lib 54 passed; övriga bin/tester
gröna (inkl. spara1302 13 passed, 2 ignored).

## Anpassning av befintliga prov

Inga. `karta_full_version` / `karta_for_ar` ger fortfarande lutning 1.

## Alt 1501 och 1502

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta9`, gren `hckarta-lutning1485`.
Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartlut2.md`
(skrivet före bygget; tal och gränser oförändrade).
Mätsätt: `kartlut-matning1500.rs` (tidsmodell 1, version 2, Aldre t = 500 000), fröna 21–40.

### Stopp (underlaget)

**Prov 5a klarade inte talen.** Underlaget: ändra inte talen; redovisa mätvärdena och stoppa.
Prov 4 (p95 ≤ 4,0 m, max ≤ 7,0 m) och prov 5b (öst/väst inom 0,5–2,0 × r1+l1) är gröna.
Prov 3, 6 och 1 oförändrade och gröna.

```
git rev-parse HEAD (kodcommit Alt 1501/1502): 2d8dc1b8fab4cd1c7b96fc4fc3090374849ed7b7
git rev-parse HEAD (bas före uppdraget): bdb14d13b0730db60a09937159fb51e8197b80df
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/flod.rs
 M n10/hckarta/tests/lutning1485.rs
```

### Vad som byggdes

- **Alt 1501:** med `--lutning 2` tar träskregeln fårans yta från **närmaste** mittlinjeruta
  (Chebyshev; vid lika avstånd den lägsta ytan) i stället för den lägsta ytan inom
  `trask_avstand`. Utan `--lutning 2`: oförändrad (lägsta inom avståndet).
- **Alt 1502:** prov 4 och 5 ersatta. Prov 4: p95 ≤ 4,0 m och max ≤ 7,0 m. Prov 5a: träsk
  per fårruta 0,24–1,20. Prov 5b: öst/väst med r2+l2 inom 0,5–2,0 × samma kvot med r1+l1
  (samma frön). Alla på fröna 21–40, mätsätt som mätprogrammet.

### Talen frö 21–40 (mätsätt kartlut-matning1500)

#### Utan Alt 1501 (lägsta ytan inom `trask_avstand`, som före)

| riktning | lutning | träsk/fårruta | öst/väst | djup p95 (m) | djup max (m) |
|--:|--:|--:|--:|--:|--:|
| 1 | 1 | 0,502 | 0,419 | 2,000 | 2,000 |
| 2 | 1 | 2,158 | 0,667 | 2,000 | 2,000 |
| 1 | 2 | 0,074 | 0,017 | 3,235 | 6,322 |
| 2 | 2 | **1,123** | 0,391 | 2,630 | 6,361 |

#### Med Alt 1501 (närmaste mittlinjeruta)

| riktning | lutning | träsk/fårruta | öst/väst | djup p95 (m) | djup max (m) |
|--:|--:|--:|--:|--:|--:|
| 1 | 1 | 0,502 | 0,419 | 2,000 | 2,000 |
| 2 | 1 | 2,158 | 0,667 | 2,000 | 2,000 |
| 1 | 2 | 0,254 | 0,040 | 3,235 | 6,322 |
| 2 | 2 | **1,674** | 0,460 | 2,630 | 6,361 |

Lutning 1 oförändrad byte för beteende (samma tal). Lutning 2: djupet oförändrat (träsk
påverkar inte mittlinjeytan); träsk/fårruta stiger (närmaste yta är ofta högre än lägsta
inom avståndet → fler rutor under yta + 1 m).

### Prov 5b-kvot och prov 6

| | värde | gräns |
|--|--:|--|
| öst/väst r2+l2 | 0,460 | — |
| öst/väst r1+l1 | 0,419 | — |
| kvot (l2 / r1l1) | **1,097** | 0,5–2,0 |
| Johnson fro=181 Aldre | 0,095934 | < 0,10 |
| Johnson fro=181 Ovre | 0,078597 | < 0,10 |

### Provutfall (Alt 1502)

| Prov | Utfall | Anmärkning |
|------|--------|------------|
| 1 utan lutning 2 | grönt | sha256 = före |
| 3 vattenyta | grönt | som förut |
| 4 djup p95/max | grönt | p95 2,630 ≤ 4,0; max 6,361 ≤ 7,0 |
| 5a träsk/fårruta | **rött** | **1,674** ∉ 0,24–1,20 |
| 5b öst/väst | grönt | 1,097 ∈ 0,5–2,0 |
| 6 Johnson | grönt | 0,096 / 0,079 < 0,10 |

### Röda proven (mutanter i kopia)

| Mutant | Prov | Utfall |
|--------|------|--------|
| Yta − 3 m extra i `fallande_yta_mittlinjer` (per iteration) | 4 | rött: p95/max ≫ 4/7 |
| `trask_avstand` dubblat (Aldre 3 → 6) | 5a | rött: träsk/fårruta 2,850 ∉ 0,24–1,20 |

Alt 1501 avslaget (lägsta ytan): träsk/fårruta r2+l2 = **1,123** (skulle ha legat i 0,24–1,20);
med Alt 1501 = **1,674** (utanför). Inget krav på rött för avslaget; talen ovan.

### Avvikelser (underlagets tal oförändrade)

1. **Prov 5a — träsk per fårruta** med riktning 2 + lutning 2, frö 21–40: uppmätt **1,674**
   (mål 0,24–1,20). Närmaste-regeln (Alt 1501) ger fler träsk än lägsta-regeln (1,123).
   Bygget stoppas här enligt underlaget; inga tal ändrade.
2. Djupet (prov 4) påverkas inte av Alt 1501 och klarar de nya gränserna på fröna 21–40.

### cargo test

hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`):

- `lutning1485`: 12 passed, **1 failed** (prov 5a); redovisning ok.
- Befintliga (fara/bredd/tid/vall/vaxt/riktning/lopp/fyll/s1–s4/granser): alla gröna.

hcsim (`CARGO_TARGET_DIR=…/n10/hcsim/target`, `--release`): lib 54 passed; övriga bin/tester
gröna (inkl. spara1302 13 passed, 2 ignored).

## Alt 1533

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta9`, gren `hckarta-lutning1485`.
Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartlut3.md`
(skrivet före körningen; banden oförändrade efter körningen).
Mätsätt: `kartlut-matning1500.rs` (tidsmodell 1, version 2, Aldre t = 500 000).
Mutanter i själva kartan via `lagg_flodnat_version_lutning_prov` → `lagg_trask_fran`
(inte mätprogrammets kopia).

### Utfall

**Grönt.** Mutanterna röda i riktning 2 på fröna 1–20; fröna 41–60 inom de låsta banden.
Grenen kan slås ihop enligt underlaget (lutning 2 blir inte standard).

```
git rev-parse HEAD (kodcommit Alt 1533): 2b79c3389fd59d5bcea5efaa36ead18f777a49dd
git rev-parse HEAD (bas före uppdraget): 85dedbe151c9256f930b8ebe4906944d14dd4eb3
```

`git status --short` före första commit (arbetsträdet rent):

```
(tomt)
```

### Vad som byggdes

- Närmaste-regeln (Alt 1501) oförändrad under `--lutning 2`.
- Prov 5a: låsta band **1,35–2,10** (riktning 2) och **0,16–0,40** (riktning 1), skrivna
  som tal; fröna 41–60. Prov 4, 5b, 6, 3 och 1 oförändrade i kriterier (4/5b på 41–60).
- `TraskProvMutant` + `lagg_flodnat_version_lutning_prov`: mutant A (närmaste := lägsta)
  och mutant B (dubblat `trask_avstand`) genom kartans `lagg_trask_fran`.
- LASMIG: banden märkta som **eget val** utan källa.

### Mutanter frö 1–20 (riktning 2 krävs röd mot 1,35–2,10)

| Variant | r2+l2 | r1+l2 | r2 mot 1,35–2,10 |
|--|--:|--:|--|
| Utan mutant | 1,619 | 0,244 | inom (referens) |
| Mutant A (närmaste := lägsta) | **1,061** | 0,072 | **rött** (under 1,35) |
| Mutant B (dubblat `trask_avstand`) | **2,820** | 0,268 | **rött** (över 2,10) |

Båda mutanterna röda i riktning 2 → körning på 41–60 tillåten. Mutant B inom bandet i
riktning 1 (känd blindhet enligt underlaget).

### Talen frö 41–60 (mätsätt kartlut-matning1500)

| riktning | lutning | träsk/fårruta | öst/väst | djup p95 (m) | djup max (m) |
|--:|--:|--:|--:|--:|--:|
| 1 | 1 | 0,507 | 0,369 | 2,000 | 2,000 |
| 2 | 1 | 2,138 | 0,670 | 2,000 | 2,000 |
| 1 | 2 | **0,275** | 0,035 | 3,233 | 6,597 |
| 2 | 2 | **1,625** | 0,482 | 2,617 | 4,508 |

### Provutfall (Alt 1533, frö 41–60 där det gäller)

| Prov | Utfall | Anmärkning |
|------|--------|------------|
| 1 utan lutning 2 | grönt | sha256 = före |
| 1b hcsim | grönt | slutsummor = före |
| 3 vattenyta | grönt | aldrig stigande med lutning 2 |
| 4 djup p95/max | grönt | p95 2,617 ≤ 4,0; max 4,508 ≤ 7,0 |
| 5a träsk/fårruta | grönt | r2 **1,625** ∈ 1,35–2,10; r1 **0,275** ∈ 0,16–0,40 |
| 5b öst/väst | grönt | kvot 1,307 ∈ 0,5–2,0 |
| 6 Johnson | grönt | 0,096 / 0,079 < 0,10 |
| 5a-mutanter r2 | grönt | A 1,061 och B 2,820 utanför 1,35–2,10 |
| 9 LASMIG | grönt | band + eget val |

### Avvikelser

Inga. Banden ändrades inte efter körningen. Små skillnader mot underlagets mätprogramtal
på frö 1–20 (1,619 vs 1,623; mutant B 2,820 vs 2,824) — samma regel i kartan, avrundning.

### cargo test

hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`):

- `lutning1485`: **15 passed**, 0 failed.
- Befintliga (fara/bredd/tid/vall/vaxt/riktning/lopp/fyll/s1–s4/granser): alla gröna.

hcsim (`CARGO_TARGET_DIR=…/n10/hcsim/target`, `--release`): lib 54 passed; övriga bin/tester
gröna (inkl. spara1302 13 passed, 2 ignored).
