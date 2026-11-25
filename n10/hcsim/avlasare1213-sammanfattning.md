# Avläsarprovet (Alt 1213 med 1218, 1215 och 1214)

Gränser ur `n10/docs/2026-11-21-lasunderlag-avlasare1213.md` (skrivna före körningen).

Total körtid: 5467.4 s

| Arm | Spegel | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |
|-----|--------|-------|--------|-------------------|-------|-----|-----|
| a_lott | nej | lar_lott | lar | 11/20 | 0.4961 | 0.3170 | 0.6843 |
| a_lott | ja | lar | lar_lott | 11/20 | 0.4961 | 0.3170 | 0.6843 |
| a_las | nej | lar_intagslasare | lar | 18/20 | 0.8437 | 0.0069 | 1.0000 |
| a_las | ja | lar | lar_intagslasare | 3/20 | 0.1666 | 0.0000 | 0.9695 |
| a_las_lott | nej | lar_intagslasare | lar_lott | 18/20 | 0.8437 | 0.0069 | 1.0000 |
| a_las_lott | ja | lar_lott | lar_intagslasare | 3/20 | 0.1666 | 0.0000 | 0.9695 |
| a_las_tab | nej | lar_intagslasare | tabell | 0/20 | 0.0871 | 0.0210 | 0.1211 |
| a_las_tab | ja | tabell | lar_intagslasare | 20/20 | 0.9160 | 0.8608 | 0.9527 |
| a_las_ls | nej | lar_intagslasare | lar_slump(0.1) | 2/20 | 0.1297 | 0.0000 | 0.9887 |
| a_las_ls | ja | lar_slump(0.1) | lar_intagslasare | 19/20 | 0.9350 | 0.0260 | 1.0000 |
| a_orakel | nej | orakel_forutsagare | lar | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| a_orakel | ja | lar | orakel_forutsagare | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| s_lar | nej | lar | slump | 0/20 | 0.0000 | 0.0000 | 0.0002 |
| s_lar | ja | slump | lar | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| s_tab | nej | tabell | slump | 0/20 | 0.0357 | 0.0162 | 0.0564 |
| s_tab | ja | slump | tabell | 20/20 | 0.9627 | 0.9280 | 0.9854 |
| s_tab_tak | nej | tabell | allvetande_slump(0.1) | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| s_tab_tak | ja | allvetande_slump(0.1) | tabell | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| s_lar_tak | nej | lar | allvetande_slump(0.1) | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| s_lar_tak | ja | allvetande_slump(0.1) | lar | 20/20 | 1.0000 | 1.0000 | 1.0000 |

## Avgöranden

- **Spegel a_lott:** medel av (andel + spegelandel) = 0.9922 (krav 1 ± 0.116) — inom gränsen.
- **Spegel a_las:** medel av (andel + spegelandel) = 1.0103 (krav 1 ± 0.116) — inom gränsen.
- **Spegel a_las_lott:** medel av (andel + spegelandel) = 1.0103 (krav 1 ± 0.116) — inom gränsen.
- **Spegel a_las_tab:** medel av (andel + spegelandel) = 1.0031 (krav 1 ± 0.116) — inom gränsen.
- **Spegel a_las_ls:** medel av (andel + spegelandel) = 1.0646 (krav 1 ± 0.116) — inom gränsen.
- **Spegel a_orakel:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel s_lar:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel s_tab:** medel av (andel + spegelandel) = 0.9983 (krav 1 ± 0.116) — inom gränsen.
- **Spegel s_tab_tak:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel s_lar_tak:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **H4 (avläsaren är felet):** a_las (lar_intagslasare mot lar) 18/20 (≥15); a_las_lott (mot lar_lott) 18/20. Speglar ok. Stämmer.
- **H4b (regeln för lika värden):** a_lott (lar_lott mot lar) 11/20 (≥15). Spegel ok. Stämmer inte.
- **H6 (förutsägaren):** a_orakel (orakel_forutsagare mot lar) 20/20 (≥15). Spegel ok. Stämmer.
- **Regel 14:** a_las_tab (lar_intagslasare mot tabell) 0/20 (under 15 betyder att förutsägaren inte bär mer än en tabell). Spegel ok. Förutsägaren bär inte mer än en tabell; ordet världsmodell används inte.
- **H5 (felet per handling):** s_lar / lar: kvot sent/tidigt flyttmedel = 0.6502 (krav ≥ 0.8), stå kvar = 1.0950 (krav ≤ 0.5). Spegel s_lar ok. Stämmer inte.
- **Stegen (Alt 1215):** s_lar 0/20 (synlig nackdel; spegel ok); s_tab 0/20 (synlig nackdel; spegel ok); s_tab_tak 0/20 (synlig nackdel; spegel ok); s_lar_tak 0/20 (synlig nackdel; spegel ok).
- **v mot läsaren (a_las):** medel andel steg där v valt annat = 0.8326.
