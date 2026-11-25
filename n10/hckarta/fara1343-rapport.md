# Rapport: Alt 1343 + Alt 1344 (flodversion 2)

Skriven 2026-11-21 av Cursor i arbetsträdet `pi-hckarta2`, gren `hckarta-fara1343`.
Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-21-lasunderlag-kartfara.md` (oförändrade tal och gränser).

## Git

```
git rev-parse HEAD (kodcommit): 4c1e46f
git rev-parse HEAD (bas före uppdraget): 5559ae30d786b4daa4b31d6bcf700b14cc54a312
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/filer.rs
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
 M n10/hckarta/src/main.rs
?? n10/hckarta/fara1343-fore.txt
?? n10/hckarta/tests/fara1343.rs
```

(`Karta` fick **inga** nya fält: andelar ligger i `FlodAndelar` utanför `Karta`, så hcsim kompilerar utan ändring.)

## Vad som byggdes

- `--flodversion 1|2` (standard 1) i `hckarta`.
- Version 2: mittlinje = version 1:s pathfinding; lager `fara_andel` (115 m, Butler 1980 [A]) och
  `balte_andel` (900 m, tak 0,85, egna val); 0 `Flodfara`; träsk/sjöar från mittlinjen;
  filer `fara_andel.f32` / `balte_andel.f32` och `"flodversion": 2` i `karta.json`.
- `karta_full` / hcsim oförändrade (alltid version 1).

## Prov 1 — version 1 orörd (sha256)

Före: `n10/hckarta/fara1343-fore.txt`. Efter: samma hashar byte för byte för båda körningarna
(`t=500000` och `t=20000`, `--fro 181`, `--skriv`).

| t | kontrollsumma |
|--:|--|
| 500000 | 9828389463751256610 |
| 20000 | 7307636836510803732 |

Inga `fara_andel.f32` / `balte_andel.f32` i version 1.

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

`--fro 181`, `ruta_m=2000`, område 64×48. Medel = Σ andel / 3072.

### Aldre (t = 500 000)

| | Land | Flodfara | Trask | Sjo | medel fara_andel | medel balte_andel |
|--|--:|--:|--:|--:|--:|--:|
| version 1 | 2225 | 640 | 146 | 61 | — | — |
| version 2 | 2822 | 0 | 189 | 61 | 0.007788 | 0.060952 |

### Ovre (t = 20 000)

| | Land | Flodfara | Trask | Sjo | medel fara_andel | medel balte_andel |
|--|--:|--:|--:|--:|--:|--:|
| version 1 | 2663 | 256 | 107 | 46 | — | — |
| version 2 | 2911 | 0 | 115 | 46 | 0.003063 | 0.023968 |

Version 2 kontrollsummor (inkl. andelslager): Aldre 3197361095185254305; Ovre 2161726019821368245.

## cargo test

### hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`)

```
fara1343.rs: 10 passed
granser.rs:   1 passed
s1.rs:        5 passed
s2.rs:        6 passed
s3.rs:        5 passed
s4.rs:        8 passed
```

Alla gröna (prov 1–9 + redovisning).

### hcsim (`CARGO_TARGET_DIR=…/n10/hcsim/target`, `--release`)

```
tests/prover.rs: 46 passed
```

Kompilerade utan ändring i `n10/hcsim/`.

## Röda felprov (medvetna fel i tillfällig kopia av `flod.rs`, sedan återställd)

| Fel | Prov | Utfall |
|-----|------|--------|
| `balte[p] = 0` och borttagen `balte ≥ fara`-höjning | prov4_ytan | rött: `balte_andel 0 < fara_andel …` |
| `fa` räknad med 230 m i stället för 115 m | prov4_ytan | rött: yta relativ avvikelse ≈ 1,0 |
| `rensa_flodfara` utkommenterad | prov5_inga_hela_flodrutor | rött: Flodfara=286 ≠ 0 |
| kandidat-`dj` ∈ {−2,0,2} i stället för {−1,0,1} | prov6_sammanhang | rött: `\|dj\| > 1` |
| andelsmärken `Last` i stället för `Simulering` | prov9_markning | rött: `left: Last right: Simulering` |

(Första försöket med bara `balte=0` läktes av steget som tvingar `balte ≥ fara` — det dokumenteras som att felet måste nå assertionen.)

## Avvikelser

Inga. Version 1:s filer och hcsim:s slutsummor oförändrade. Inget utanför `n10/hckarta/` ändrat.
Underlagets tal (115 m, 900 m, 0,85, relativa 1e-6) oförändrade.
