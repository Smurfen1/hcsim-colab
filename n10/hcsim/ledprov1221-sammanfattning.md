# Ledprovet (Alt 1221, med K Alt 1220 och känslighetsprovet Alt 1224)

Gränser ur `n10/docs/2026-11-21-lasunderlag-ledprov1221.md` (skrivna före körningen).

Total körtid: 3215.6 s

| Arm | Spegel | Märkt | Omärkt | Återväxt | Vunna (andel>0,5) | Medel | Min | Max |
|-----|--------|-------|--------|----------|-------------------|-------|-----|-----|
| o_slump | nej | orakel_forutsagare | slump | 1 | 0/20 | 0.0153 | 0.0003 | 0.0286 |
| o_slump | ja | slump | orakel_forutsagare | 1 | 20/20 | 0.9829 | 0.9734 | 0.9944 |
| o_tab | nej | orakel_forutsagare | tabell | 1 | 0/20 | 0.2876 | 0.1217 | 0.4012 |
| o_tab | ja | tabell | orakel_forutsagare | 1 | 20/20 | 0.7051 | 0.5365 | 0.8712 |
| o_tak | nej | orakel_forutsagare | allvetande_slump(0.1) | 1 | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| o_tak | ja | allvetande_slump(0.1) | orakel_forutsagare | 1 | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| k_las | nej | lar_intagslasare | slump | 1 | 0/20 | 0.0085 | 0.0000 | 0.0184 |
| k_las | ja | slump | lar_intagslasare | 1 | 20/20 | 0.9928 | 0.9819 | 1.0000 |
| v1_tab | nej | tabell | slump | 1 | 0/20 | 0.0362 | 0.0235 | 0.0540 |
| v1_tab | ja | slump | tabell | 1 | 20/20 | 0.9667 | 0.9293 | 0.9855 |
| v05_tab | nej | tabell | slump | 0.5 | 0/20 | 0.0403 | 0.0185 | 0.0700 |
| v05_tab | ja | slump | tabell | 0.5 | 20/20 | 0.9589 | 0.9310 | 1.0000 |
| v05_ork | nej | orakel_forutsagare | slump | 0.5 | 0/20 | 0.0122 | 0.0000 | 0.0373 |
| v05_ork | ja | slump | orakel_forutsagare | 0.5 | 20/20 | 0.9873 | 0.9716 | 1.0000 |
| v2_tab | nej | tabell | slump | 2 | 0/20 | 0.0311 | 0.0207 | 0.0434 |
| v2_tab | ja | slump | tabell | 2 | 20/20 | 0.9674 | 0.9554 | 0.9777 |
| v2_ork | nej | orakel_forutsagare | slump | 2 | 0/20 | 0.0193 | 0.0143 | 0.0272 |
| v2_ork | ja | slump | orakel_forutsagare | 2 | 20/20 | 0.9804 | 0.9723 | 0.9901 |

Loggkörning `r_lar`: 20 frön, 1000000 loggrader totalt.

## Avgöranden

- **Spegel o_slump:** medel av (andel + spegelandel) = 0.9982 (krav 1 ± 0.116) — inom gränsen.
- **Spegel o_tab:** medel av (andel + spegelandel) = 0.9926 (krav 1 ± 0.116) — inom gränsen.
- **Spegel o_tak:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel k_las:** medel av (andel + spegelandel) = 1.0013 (krav 1 ± 0.116) — inom gränsen.
- **Spegel v1_tab:** medel av (andel + spegelandel) = 1.0029 (krav 1 ± 0.116) — inom gränsen.
- **Spegel v05_tab:** medel av (andel + spegelandel) = 0.9992 (krav 1 ± 0.116) — inom gränsen.
- **Spegel v05_ork:** medel av (andel + spegelandel) = 0.9994 (krav 1 ± 0.116) — inom gränsen.
- **Spegel v2_tab:** medel av (andel + spegelandel) = 0.9986 (krav 1 ± 0.116) — inom gränsen.
- **Spegel v2_ork:** medel av (andel + spegelandel) = 0.9997 (krav 1 ± 0.116) — inom gränsen.
- **K (H4 står eller stryks):** k_las (lar_intagslasare mot slump) 0/20 (≥15). Spegel ok. H4 stryks (läsarens seger i Alt 1213 var slump); B förblir stängd.
- **Oraklet mot slumpen:** o_slump 0/20 (under 15: G före hjärnprov). Spegel ok. inte ens sann information slår spridningen; då går G (världens trängsel och bete, med källa) före alla hjärnprov. o_tab 0/20 (spegel ok); o_tak 0/20 (spegel ok).
- **R²-reglerna (medel över de 8 flyttarna):**
- **G1:** medel flytt = 0.9969, stå kvar = 1.0000.
  - handling 0: R² = 1.0000 (n_anp=196665, n_prov=195266)
  - handling 1: R² = 0.9970 (n_anp=38754, n_prov=37918)
  - handling 2: R² = 0.9961 (n_anp=37923, n_prov=36809)
  - handling 3: R² = 0.9979 (n_anp=36128, n_prov=37823)
  - handling 4: R² = 0.9966 (n_anp=38754, n_prov=39304)
  - handling 5: R² = 0.9974 (n_anp=38625, n_prov=38471)
  - handling 6: R² = 0.9965 (n_anp=37320, n_prov=37228)
  - handling 7: R² = 0.9979 (n_anp=37308, n_prov=37959)
  - handling 8: R² = 0.9962 (n_anp=38523, n_prov=39222)
- **G2:** medel flytt = 0.0001, stå kvar = 0.0001.
  - handling 0: R² = 0.0001 (n_anp=196665, n_prov=195266)
  - handling 1: R² = -0.0004 (n_anp=38754, n_prov=37918)
  - handling 2: R² = 0.0003 (n_anp=37923, n_prov=36809)
  - handling 3: R² = -0.0002 (n_anp=36128, n_prov=37823)
  - handling 4: R² = 0.0002 (n_anp=38754, n_prov=39304)
  - handling 5: R² = 0.0003 (n_anp=38625, n_prov=38471)
  - handling 6: R² = 0.0001 (n_anp=37320, n_prov=37228)
  - handling 7: R² = -0.0001 (n_anp=37308, n_prov=37959)
  - handling 8: R² = 0.0003 (n_anp=38523, n_prov=39222)
- **G3:** medel flytt = 0.3398, stå kvar = 0.9952.
  - handling 0: R² = 0.9952 (n_anp=196665, n_prov=195266)
  - handling 1: R² = 0.2997 (n_anp=38754, n_prov=37918)
  - handling 2: R² = 0.3773 (n_anp=37923, n_prov=36809)
  - handling 3: R² = 0.3380 (n_anp=36128, n_prov=37823)
  - handling 4: R² = 0.3464 (n_anp=38754, n_prov=39304)
  - handling 5: R² = 0.3236 (n_anp=38625, n_prov=38471)
  - handling 6: R² = 0.3682 (n_anp=37320, n_prov=37228)
  - handling 7: R² = 0.3310 (n_anp=37308, n_prov=37959)
  - handling 8: R² = 0.3342 (n_anp=38523, n_prov=39222)
  Avgörande: G1 minst 0,5 och G2 minst 0,2 lägre än G1: konkret fel i hjärnans värdering (Alt 1212); B får öppnas.
- **Känslighetsprovet (Alt 1224):** v05_tab 0/20, v1_tab 0/20, v2_tab 0/20; v05_ork 0/20, o_slump 0/20, v2_ork 0/20. Speglar ok. ingen sidbyte mellan återväxt 0,5/1/2 (tabell–slump och orakel–slump).

## Loggfil

`ledprov1221-logg.tsv` är 242 MB (över 50 MB) och läggs inte i git.
SHA-256: `02d7bdffd22b04ad8febee5295790471b52d6f202d0338ed8e1de971ab1cada4`.

## Rödhetsprov (kontroller 1–5)

Varje nytt prov prövades rött med ett fel med flit (sedan återställt):

1. `r2_pa_kant_samband`: kravet ändrat till R² > 1,01 → rött (fick ≈0,99995).
2. `r2_omkastade_mal`: kravet ändrat till R² > 0,5 → rött (fick ≈−0,74).
3. `r2_skilda_fron_vagrar_overlap`: förväntade Ok vid överlapp → rött.
4. `tom_logg_stoppar`: förväntade Ok → rött. `loggen_andrar_inget`: assert_ne på djur_summa → rött (summorna var lika).
5. `atervaxt_faktorn_nar_simuleringen`: mutanten "faktorn läses inte" (R_V/SOL utan `atervaxt`) → rött (faktor 2 = faktor 1).
6. De 26 tidigare proven gröna tillsammans med de nya (32 totalt i `prover.rs`).
