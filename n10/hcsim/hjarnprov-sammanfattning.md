# Hjärnprovet (Alt 1200 med 1201 och 1202)

Gränser ur `n10/docs/2026-11-21-lasunderlag-hjarnprov.md` (skrivna före körningen).

Total körtid: 2509.0 s

| Arm | Spegel | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |
|-----|--------|-------|--------|-------------------|-------|-----|-----|
| arm2 | nej | frusen | slump | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| arm1 | nej | lar | frusen | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| arm3 | nej | lar | tabell | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| arm2 | ja | slump | frusen | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| arm1 | ja | frusen | lar | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| arm3 | ja | tabell | lar | 20/20 | 1.0000 | 0.9991 | 1.0000 |
| trappa_0.99 | nej | rabatt(0.99) | lar | 18/20 | 0.6391 | 0.3610 | 0.8559 |
| trappa_0.97 | nej | rabatt(0.97) | lar | 20/20 | 0.7896 | 0.5567 | 0.9673 |
| trappa_0.95 | nej | rabatt(0.95) | lar | 20/20 | 0.9405 | 0.8466 | 0.9931 |
| tak | nej | allvetande | slump | 0/20 | 0.1225 | 0.0000 | 0.2177 |

## Avgöranden

- **Arm 2 (frusen mot slump):** 0 av 20 frön — synlig nackdel.
- **Spegel arm2:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel arm1:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel arm3:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Arm 1 (lärande mot frusen):** 20 av 20 frön — synlig fördel. Tolkas.
- **Arm 3 (lärande mot tabell):** 0 av 20 frön — synlig nackdel. Tolkas.
- **Trappa (0,99 / 0,97 / 0,95):** vunna 18 / 20 / 20. Instrumentets gräns: 0.99. Följden är monoton.
- **Tak (allvetande mot slump):** 0 av 20 frön. Allvetande ger inte minst 15: världen belönar inte kunskap om växterna; nej i arm 1 och 3 kan bero på världen.

**Ord:** inte visat — villkoren för ordet världsmodell är inte uppfyllda (regel 14).
