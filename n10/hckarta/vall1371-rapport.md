# Rapport: Alt 1371 (strandvallar i flodversion 2)

Skriven 2026-11-21 av Cursor i arbetsträdet `pi-hckarta5`, gren `hckarta-vall1371`.
Underlag: `/home/hakan/sandbox/pi-hckarta5/n10/docs/2026-11-21-lasunderlag-kartvall.md`
(skrivet före bygget; tal och gränser oförändrade).

## Git

```
git rev-parse HEAD (kodcommit): 5fe0bd1a91f7a39540aa925b1c9676db0b789985
git rev-parse HEAD (bas före uppdraget): 1905f603d20c975ac80eac488fd0cdc87839551b
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
?? n10/hckarta/tests/data/vall1371-alt1386-v2/
?? n10/hckarta/tests/data/vall1371-fore-v2-tm*-f*-t*-*
?? n10/hckarta/tests/vall1371.rs
?? n10/hckarta/vall1371-fore.txt
?? n10/hckarta/vall1371-rapport.md
```

## Vad som byggdes

- Version 2: `v_f = min(600, 4 · w_f)`; `vall_andel += v_f · L / ruta_m²` längs levande
  mittlinjer, sedan `vall_andel = min(vall_andel, 1 − balte_andel)`; `vall_hojd_m` = 2,0 där
  `vall_andel > 0`, annars 0. `hojd` orörd.
- Filer `vall_andel.f32` / `vall_hojd.f32` i `karta.json`; `foror.tsv` får kolumnen `vall_m`.
- `har_vall`: mappar från Alt 1343 (utan `foror.tsv`) och Alt 1386 (utan vallfiler) läses;
  kontrollsumman tar med vallagren och `vall_m` bara när `har_vall`.
- Konstanter: `VALL_FAKTOR = 4`, `VALL_MAX_M = 600`, `VALL_HOJD_M = 2,0`. LASMIG uppdaterad
  (Butler [A] för 600 m / 2 m / 1–3 m; faktor 4, båda sidorna, övergivna utan vall = eget val;
  genombrott byggs inte).

## Anpassning av befintliga prov

| Fil | Påstående | Ändring | Varför |
|-----|-----------|---------|--------|
| `tests/bredd1386.rs` prov8 | `d.len() == 4` | `== 5` + assert `vall_m` | `foror.tsv` fick `vall_m` |
| `tests/bredd1386.rs` prov10 | märken | assert `har_vall` / `vall_andel` | nya lager |
| `tests/fara1343.rs` prov7 | filförteckning | `vall_andel.f32`, `vall_hojd.f32` | samma frö / annat frö |
| `tests/fara1343.rs` prov8–9 | v1/v2-filer, märken | assert vallfiler saknas i v1, finns i v2; Simulering | Alt 1371 |
| `tests/tid1401.rs` prov1 | v2-filförteckning | utelämnar `karta.json` och `foror.tsv`; behåller `fara_andel`/`balte_andel` | de ändras med flit av vall |

## Prov 1 — version 1 orörd (sha256)

Före: `n10/hckarta/vall1371-fore.txt`. Efter: samma hashar byte för byte för
`--t 500000/20000/0`, `--flodversion 1`, båda `--tidsmodell 1|2`, `--fro 181`, `--skriv`.

| tidsmodell | t | kontrollsumma |
|-----------:|--:|--:|
| 1 | 500000 | 9828389463751256610 |
| 1 | 20000 | 7307636836510803732 |
| 1 | 0 | 9344846268250959607 |
| 2 | 500000 | 9828389463751256610 |
| 2 | 20000 | 9191286358991072263 |
| 2 | 0 | 7583268903656465661 |

Inga `vall_*.f32` / `foror.tsv` i version 1.

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
`fara_andel`, `balte_andel`, `hojd` och mittlinjemask (`fara_andel > 0`) byte-identiska med
före-filerna under `tests/data/vall1371-fore-*`.

## Redovisade tal (räknade av programmet)

`--fro 181`, version 2, `--tidsmodell 2`, `ruta_m=2000`, 64×48. Medel = Σ `vall_andel` / 3072.

### t = 500 000 (Aldre, levande 0–4)

| fara | w_f | v_f | rutor |
|-----:|----:|----:|------:|
| 0 | 166.929757 | 600.000000 | 64 |
| 1 | 149.202516 | 596.810066 | 64 |
| 2 | 146.881059 | 587.524235 | 64 |
| 3 | 238.418326 | 600.000000 | 64 |
| 4 | 176.940862 | 600.000000 | 64 |

Σ `vall_andel` / 3072 = **0,038946**; rutor med vall = **320**; rutor där klippningen mot
`1 − balte_andel` slog till = **46**. Kontrollsumma (efter, inkl. vall) = 6011415087537404248.

### t = 20 000 (Ovre, levande 1 3)

| fara | w_f | v_f | rutor |
|-----:|----:|----:|------:|
| 1 | 149.202516 | 596.810066 | 64 |
| 3 | 238.418326 | 600.000000 | 64 |

Σ `vall_andel` / 3072 = **0,014581**; rutor med vall = **128**; klippning = **46**.
Kontrollsumma = 10345612022611217897.

## Handräkning (prov 4, underlagets tal)

- Rak (dj = 0), w = 100 m, ruta 2000 m: `v_f` = 400; `vall_andel` = 0,2.
- Diagonal (dj = ±1), w = 320 m: `v_f` = 600 (tak); före klippning 0,424264… (inom 1e-4 av 0,4243);
  med `balte_andel` = 0,85 → `vall_andel` = 0,15 exakt (inom 1e-6).

## cargo test

### hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`)

```
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
tests/prover.rs: 46 passed
```

Kompilerade utan ändring i `n10/hcsim/` (länkade om mot ny hckarta).

## Röda felprov (medvetna fel i tillfällig kopia av `flod.rs`, sedan återställd)

| Fel | Prov | Utfall |
|-----|------|--------|
| Utan klippning mot `1 − balte_andel` | prov5_summan | rött: `balte+vall > 1` (t.ex. 0,674+0,424) |
| Utan taket 600 m (`v_f = 4 · w_f`) | prov6_taket | rött: `v_f 803… > 600` |
| Vallar längs alla fem fåror (tidsmodell 2) | prov8_levande_foror | rött: övergiven mittlinje har vall |

## Avvikelser

Inga. Version 1:s filer, version 2:s gamla lager och hcsim:s slutsummor oförändrade.
Inget utanför `n10/hckarta/` ändrat. Underlagets tal (600, 4, 2,0, handräkning, klippning)
oförändrade.

## Tillägg vid Claudes granskning (2026-11-21)
Prov 6 och 7 jämförde med programmets egna konstanter (`VALL_FAKTOR`, `VALL_MAX_M`, `VALL_HOJD_M`) och kunde därför inte bli röda när konstanten
ändrades: felet "vallens höjd 3 m" gav inget rött prov. Proven jämför nu med underlagets tal (4, 600 m, 2,0 m). Prövat: höjden 3 m → prov 7 rött;
faktorn 5 → prov 4 och 6 röda. Alla prov gröna efteråt (hckarta 69, hcsim 47).

