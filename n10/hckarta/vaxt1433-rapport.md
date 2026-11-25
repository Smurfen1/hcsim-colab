# Rapport: Alt 1433 (växtgrupper med höjd och täthet per ruta)

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta6`, gren `hckarta-vaxt1433`.
Underlag: `/home/hakan/sandbox/pi-hckarta6/n10/docs/2026-11-22-lasunderlag-kartvaxt.md`
(skrivet före bygget; tal och gränser oförändrade).

## Git

```
git rev-parse HEAD (kodcommit): 5e50511ce02799ccda0ac29be083cdef1cc45154
git rev-parse HEAD (bas före uppdraget): 3dab46a200aaa6f71a44b8b88a7e87e5f2a719d7
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/filer.rs
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
 M n10/hckarta/src/main.rs
 M n10/hckarta/tests/bredd1386.rs
 M n10/hckarta/tests/fara1343.rs
 M n10/hckarta/tests/tid1401.rs
 M n10/hckarta/tests/vall1371.rs
?? n10/hckarta/src/vaxt.rs
?? n10/hckarta/tests/data/vaxt1433-fore-v2-tm*-f*-t*-*
?? n10/hckarta/tests/vaxt1433.rs
?? n10/hckarta/vaxt1433-fore.txt
?? n10/hckarta/vaxt1433-rapport.md
```

## Vad som byggdes

- Flodversion 2: täthet per ruta för sex grupper ur vattentyp, period och (för örter i
  Aldre) `balte_andel` / `vall_andel`; filer `vaxt_tathet.f32` (6·b·h f32 LE) och
  `vaxtgrupper.json` (namn, höjd, märke, artantal).
- Artkopplingen läses ur inbäddad `vaxtgrupper.tsv` (Claude; oförändrad).
- `har_vaxtgrupper`: mappar från Alt 1343/1386/1371 utan de nya filerna läses;
  kontrollsumman tar med täthet + höjd/arter bara när `har_vaxtgrupper`.
- Det gamla lagret `vaxt` orört. Version 1 orörd. hcsim orörd.
- LASMIG: varje höjd och täthet som **eget val**; Johnson 2002 [A] för citaten.

## Anpassning av befintliga prov

| Fil | Påstående | Ändring | Varför |
|-----|-----------|---------|--------|
| `tests/fara1343.rs` prov7 | filförteckning / annat frö | `vaxt_tathet.f32` + `vaxtgrupper.json`; json lika oberoende av frö | nya filer; artantal beror inte på frö |
| `tests/fara1343.rs` prov8–9 | v1/v2-filer, märken | assert växtfiler saknas i v1, finns i v2; `har_vaxtgrupper` | Alt 1433 |
| `tests/bredd1386.rs` prov10 | märken | assert `har_vaxtgrupper` / `vaxt_tathet` | nya lager |
| `tests/tid1401.rs` prov1 | kommentar | karta.json ändras även av växtgrupper | samma undantag som vall |
| `tests/vall1371.rs` prov1/9/10 | filer / märken | assert inga växtfiler i v1; finns i ny v2; gamla mappar utan växter | Alt 1433 |

## Prov 1 — version 1 orörd (sha256)

Före: `n10/hckarta/vaxt1433-fore.txt`. Efter: samma hashar byte för byte för
`--t 500000/20000/0`, `--flodversion 1`, båda `--tidsmodell 1|2`, `--fro 181`, `--skriv`.

| tidsmodell | t | kontrollsumma |
|-----------:|--:|--:|
| 1 | 500000 | 9828389463751256610 |
| 1 | 20000 | 7307636836510803732 |
| 1 | 0 | 9344846268250959607 |
| 2 | 500000 | 9828389463751256610 |
| 2 | 20000 | 9191286358991072263 |
| 2 | 0 | 7583268903656465661 |

Inga `vaxt_tathet.f32` / `vaxtgrupper.json` i version 1.

## Prov 2 — hcsim orörd

Kommando: `hcsim kor --lat0 47 --lon0 -108 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 181 --ar 5 --n0 400`

| | före | efter |
|--|--|--|
| slut_antal | 4992 | 4992 |
| djur_summa | 9527944887746424482 | 9527944887746424482 |
| karta_summa | 9828389463751256610 | 9828389463751256610 |
| fodda / dod_svalt / dod_alder | 27785 / 23125 / 68 | samma |
| v_medel | 82.54336761406384 | samma |

## Prov 3 — version 2:s gamla lager orörda

För fröna 181–183, t = 500000/20000/0, båda tidsmodellerna: `vattentyp`, `vatten_yta`,
`fara_andel`, `balte_andel`, `vall_andel`, `vall_hojd`, `foror.tsv` och `vaxt` byte-identiska
med före-filerna under `tests/data/vaxt1433-fore-*` (och sha256 i `vaxt1433-fore.txt`).
`karta.json` och kontrollsumman ändras med flit (växtlager + ny summa).

## Redovisade tal (räknade av programmet)

`--fro 181`, version 2, `--tidsmodell 2`, `ruta_m=2000`, 64×48.

### t = 500 000 (Aldre)

| grupp | medeltäthet | arter |
|-------|------------:|------:|
| lovtrad | 0,459310 | 2 |
| barrtrad | 0,015381 | 2 |
| ort | 0,118848 | 0 |
| ormbunke | 0,021449 | 4 |
| kottepalm_ginkgo | 0,009186 | 1 |
| vattenvaxt | 0,012109 | 1 |

Johnson-kvot = **0,073723** (&lt; 0,10). Land-rutor med örttäthet 0,30 = **320**.
Kontrollsumma (efter, inkl. växt) = 9299547147549356579.

### t = 20 000 (Ovre)

| grupp | medeltäthet | arter |
|-------|------------:|------:|
| lovtrad | 0,446289 | 2 |
| barrtrad | 0,023275 | 2 |
| ort | 0,098568 | 0 |
| ormbunke | 0,022507 | 4 |
| kottepalm_ginkgo | 0,008926 | 1 |
| vattenvaxt | 0,013607 | 1 |

Johnson-kvot = **0,091245** (&lt; 0,10). Land-rutor med örttäthet 0,30 = **0**.
Kontrollsumma = 12088502067399578334.

## cargo test

### hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`)

```
vaxt1433.rs:  11 passed
vall1371.rs:  11 passed
bredd1386.rs: 11 passed
fara1343.rs:  10 passed
tid1401.rs:   12 passed
granser.rs:    1 passed
s1.rs:         5 passed
s2.rs:         6 passed
s3.rs:         5 passed
s4.rs:         8 passed
```

Alla gröna (prov 1–10 + redovisning).

### hcsim (`CARGO_TARGET_DIR=…/n10/hcsim/target`, `--release`)

```
tests/prover.rs:    54 passed
tests/spara1302.rs: 13 passed (2 ignored)
```

Kompilerade utan ändring i `n10/hcsim/` (länkade om mot ny hckarta).

## Röda felprov (medvetna fel i tillfällig kopia av `vaxt.rs`, sedan återställd)

| Fel | Prov | Utfall |
|-----|------|--------|
| Örter vid fåran i alla perioder (utan Aldre-villkor) | prov4_tabellen | rött: Ovre Land vid fåra fick 0,30 i stället för 0,10 |
| Barrträd 0,25 också på Land | prov6_johnsons_grans | rött: Johnson-kvot 0,323 &gt; 0,10 |
| Höjd lovtrad 16,0 i stället för 15,0 | prov7_hojderna | rött: fick 16, väntade 15 |

## Avvikelser

Inga. Version 1:s filer, version 2:s gamla lager och hcsim:s slutsummor oförändrade.
Inget utanför `n10/hckarta/` ändrat. Underlagets tal (höjder, tätheter, Johnson-gräns 0,10,
örttillägg bara i Aldre) oförändrade.
