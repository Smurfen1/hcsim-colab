# Rapport: Alt 1386 (fårans bredd per fåra i flodversion 2)

Skriven 2026-11-21 av Cursor i arbetsträdet `pi-hckarta3`, gren `hckarta-bredd1386`.
Underlag: `/home/hakan/sandbox/pi-hckarta3/n10/docs/2026-11-21-lasunderlag-kartbredd.md` (oförändrade tal och gränser).
Byggs enligt Håkans Alt 1392.

## Git

```
git rev-parse HEAD (kodcommit): 145946690cfd3f1bc82f02b654a2de85eb72544a
git rev-parse HEAD (bas före uppdraget): 12d3c73dddb9c251267103132ad9fb3be8395cad
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/filer.rs
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
 M n10/hckarta/src/main.rs
 M n10/hckarta/tests/fara1343.rs
?? n10/hckarta/bredd1386-fore.txt
?? n10/hckarta/tests/bredd1386.rs
?? n10/hckarta/tests/data/
?? n10/hckarta/bredd1386-rapport.md
```

## Vad som byggdes

- Version 2: varje fåra får `w_f = exp(ln 100 + u_f · (ln 320 − ln 100))` ur egen SplitMix64-ström
  (`fro ^ 0x4252_4544_4431_3338`); `b_f = 4 · w_f`; tak 0,85 kvar.
- Vägsökningens och sjöarnas `rng` orörd → mittlinjer, träsk och sjöar oförändrade.
- `foror.tsv` (kolumner `fara`, `bredd_m`, `balte_m`, `rutor`) skrivs i version 2 och listas i `karta.json`.
- Version 1 skriver inte filen. `las_3d` / `las_3d_med` läser Alt 1343-mappar utan `foror.tsv`.
- Kontrollsumma version 2 tar med bredderna när `foror` inte är tom; version 1 oförändrad.
- Konstanter: `FARA_BREDD_MIN_M = 100`, `FARA_BREDD_MAX_M = 320`, `BALTE_FAKTOR = 4`;
  `FARA_BREDD_M` / `BALTE_BREDD_M` borttagna. LASMIG omskriven (Butler 115 m-raden borta).

## Anpassning av `tests/fara1343.rs`

| Påstående | Ändring | Varför |
|-----------|---------|--------|
| `FARA_BREDD_M` i ytombyggnad (prov4) | bredd per fåra ur `a.foror[fi].bredd_m` | konstanten borttagen; ytan beror på fårans egna w |
| `FARA_BREDD_M < ruta_m` (prov5) | `FARA_BREDD_MAX_M < ruta_m` | maxbredd 320 m räcker för att garantera 0 Flodfara |
| v1/v2-filer (prov1, 7, 8) | assert `foror.tsv` saknas i v1, finns i v2 | ny fil i Alt 1386 |

## Prov 1 — version 1 orörd (sha256)

Före: `n10/hckarta/bredd1386-fore.txt`. Efter: samma hashar byte för byte för båda körningarna
(`t=500000` och `t=20000`, `--fro 181`, `--skriv`).

| t | kontrollsumma |
|--:|--|
| 500000 | 9828389463751256610 |
| 20000 | 7307636836510803732 |

Inga `fara_andel.f32` / `balte_andel.f32` / `foror.tsv` i version 1.

## Prov 2 — hcsim orörd

Kommando: `hcsim kor --lat0 47 --lon0 -108 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 181 --ar 5 --n0 400`

| | före | efter |
|--|--|--|
| slut_antal | 4992 | 4992 |
| djur_summa | 9527944887746424482 | 9527944887746424482 |
| karta_summa | 9828389463751256610 | 9828389463751256610 |
| fodda / dod_svalt / dod_alder | 27785 / 23125 / 68 | samma |
| v_medel | 82.54336761406384 | samma |

## Prov 3 — mittlinjer, träsk och sjöar

För fröna 181, 182, 183 och båda perioderna: `vattentyp.u8`, `vatten_yta.f32` och mittlinjemask
(`fara_andel > 0`) byte-identiska med före-filerna under `tests/data/bredd1386-fore-*`.

## Redovisade tal (räknade av programmet)

`--fro 181`, `ruta_m=2000`, område 64×48. Medel = Σ andel / 3072.

### Aldre (t = 500 000)

| | Land | Flodfara | Trask | Sjo | medel fara_andel | medel balte_andel | kontrollsumma |
|--|--:|--:|--:|--:|--:|--:|--:|
| version 2 **före** | 2822 | 0 | 189 | 61 | 0.007788 | 0.060952 | 3197361095185254305 |
| version 2 **efter** | 2822 | 0 | 189 | 61 | 0.011903 | 0.047610 | 10464361137633183643 |

Fåror efter (bredd_m / balte_m / rutor):

| fara | bredd_m | balte_m | rutor |
|-----:|--------:|--------:|------:|
| 0 | 166.929757 | 667.719027 | 64 |
| 1 | 149.202516 | 596.810066 | 64 |
| 2 | 146.881059 | 587.524235 | 64 |
| 3 | 238.418326 | 953.673305 | 64 |
| 4 | 176.940862 | 707.763446 | 64 |

### Ovre (t = 20 000)

| | Land | Flodfara | Trask | Sjo | medel fara_andel | medel balte_andel | kontrollsumma |
|--|--:|--:|--:|--:|--:|--:|--:|
| version 2 **före** | 2911 | 0 | 115 | 46 | 0.003063 | 0.023968 | 2161726019821368245 |
| version 2 **efter** | 2911 | 0 | 115 | 46 | 0.004209 | 0.016838 | 3941459844937214511 |

Fåror efter: samma bredder som fåra 0–1 ovan (samma frö, egen breddström).

### Spridning över fröna 1–200 (båda perioderna)

| min | median | max | n |
|----:|-------:|----:|--:|
| 100.316022 | 178.860006 | 319.668110 | 1400 |

(Underlagets gränser: min &lt; 150, max &gt; 250, median 160–220 — uppfyllda.)

## Handräkning (prov 6, underlagets tal)

- Rak (dj = 0), w = 100 m, ruta 2000 m: `fara_andel` = 0,05; `balte_andel` = 0,2.
- Diagonal (dj = ±1), w = 320 m: `fara_andel` = 0,226274… (inom 1e-4 av 0,2263);
  `balte_andel` före tak = 0,905097… (inom 1e-4 av 0,9051); efter tak = `BALTE_MAX` (0,85).

## cargo test

### hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`)

```
bredd1386.rs: 11 passed
fara1343.rs:  10 passed
granser.rs:    1 passed
s1.rs:         5 passed
s2.rs:         6 passed
s3.rs:         5 passed
s4.rs:         8 passed
```

Alla gröna (prov 1–10 + redovisning).

### hcsim (`CARGO_TARGET_DIR=…/n10/hcsim/target`, `--release`)

```
tests/prover.rs: 46 passed
```

Kompilerade utan ändring i `n10/hcsim/`.

## Röda felprov (medvetna fel i tillfällig kopia av `flod.rs`, sedan återställd)

| Fel | Prov | Utfall |
|-----|------|--------|
| Bredd (`u_f`) dragen ur vägsökningens/sjöarnas `rng` före sjöarna | prov3_mittlinjer_oforandrade | rött: `vattentyp` skiljer sig (sjöarna flyttade) |
| Konstant `w = 115` i stället för log-jämn dragning | prov5_spridning | rött: saknar bredd &lt; 150 / &gt; 250 |
| Fast `balte_m = 900` i stället för `4 · w` | prov4_granser | rött: `b_f ≠ 4 · w_f` |

(Handräkningsprovet 6 är ren formel och rörs inte av kartmutanter; underlagets ”prov 6”-hänvisning för 900 m-mutanten täcks av gränsprovet som kräver `b_f = 4 · w_f`.)

## Avvikelser

Inga. Version 1:s filer och hcsim:s slutsummor oförändrade. Mittlinjer/träsk/sjöar oförändrade.
Inget utanför `n10/hckarta/` ändrat. Underlagets tal (100, 320, faktor 4, tak 0,85, handräkning)
oförändrade.
