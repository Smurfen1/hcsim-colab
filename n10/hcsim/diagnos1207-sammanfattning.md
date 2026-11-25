# Diagnosen av hjärnan (Alt 1207 med 1208 och 1209)

Gränser ur `n10/docs/2026-11-21-lasunderlag-diagnos1207.md` (skrivna före körningen).

Total körtid: 3743.2 s

| Arm | Spegel | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |
|-----|--------|-------|--------|-------------------|-------|-----|-----|
| d_arm2 | nej | frusen | slump | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| d_arm1 | nej | lar | frusen | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| d_arm3 | nej | lar | tabell | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| d_tak | nej | allvetande | slump | 0/20 | 0.1103 | 0.0000 | 0.2013 |
| tak_a | nej | vaxt_per_djur | slump | 0/20 | 0.0757 | 0.0000 | 0.2005 |
| tak_b10 | nej | allvetande_slump(0.1) | slump | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| tak_b30 | nej | allvetande_slump(0.3) | slump | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| tak_b50 | nej | allvetande_slump(0.5) | slump | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| u1 | nej | lar_slump(0.1) | tabell | 0/20 | 0.0001 | 0.0000 | 0.0016 |
| u2 | nej | tabell_utan_slump | tabell | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| u3 | nej | tabell_utan_slump | lar | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| d_arm2 | ja | slump | frusen | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| d_arm1 | ja | frusen | lar | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| d_arm3 | ja | tabell | lar | 20/20 | 1.0000 | 0.9998 | 1.0000 |
| d_tak | ja | slump | allvetande | 20/20 | 0.9059 | 0.7712 | 1.0000 |
| tak_a | ja | slump | vaxt_per_djur | 20/20 | 0.9450 | 0.8014 | 1.0000 |
| tak_b10 | ja | slump | allvetande_slump(0.1) | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| tak_b30 | ja | slump | allvetande_slump(0.3) | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| tak_b50 | ja | slump | allvetande_slump(0.5) | 0/20 | 0.0000 | 0.0000 | 0.0000 |
| u1 | ja | tabell | lar_slump(0.1) | 20/20 | 0.9998 | 0.9956 | 1.0000 |
| u2 | ja | tabell | tabell_utan_slump | 20/20 | 1.0000 | 1.0000 | 1.0000 |
| u3 | ja | lar | tabell_utan_slump | 20/20 | 1.0000 | 1.0000 | 1.0000 |

## Avgöranden

- **Spegel d_arm2:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel d_arm1:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel d_arm3:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel d_tak:** medel av (andel + spegelandel) = 1.0162 (krav 1 ± 0.116) — inom gränsen.
- **Spegel tak_a:** medel av (andel + spegelandel) = 1.0207 (krav 1 ± 0.116) — inom gränsen.
- **Spegel tak_b10:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel tak_b30:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel tak_b50:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel u1:** medel av (andel + spegelandel) = 0.9999 (krav 1 ± 0.116) — inom gränsen.
- **Spegel u2:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Spegel u3:** medel av (andel + spegelandel) = 1.0000 (krav 1 ± 0.116) — inom gränsen.
- **Taket (Alt 1208):** tak_a 0/20 (synlig nackdel); tak_b10 20/20 (synlig fördel); tak_b30 20/20 (synlig fördel); tak_b50 20/20 (synlig fördel). Taket vinner: minst en arm når ≥15 av 20 (världen belönar kunskap om växterna).
- **H1 (trängsel):** allvetande andra-djur-medel = 218.5266, slump = 1.3916, kvot = 157.0332 (krav ≥ 1.5). Spegel d_tak ok. Stämmer.
- **H2 (frusen stilla/upprepar):** stå kvar = 0.4323 (krav ≥ 0.5), entropi = 1.8617 (krav < 1.0986 = ½·ln9). Spegel d_arm2 ok. Stämmer inte.
- **H3 (utforskning):** u2 (tabell_utan_slump mot tabell) 0/20 (krav ≤ 5); u1 (lar_slump(0.1) mot tabell) 0/20 (krav ≥ 6). Speglar ok. Stämmer inte.
- **d_arm2:** 0 av 20 frön — synlig nackdel. Spegel ok.
- **d_arm1:** 20 av 20 frön — synlig fördel. Spegel ok.
- **d_arm3:** 0 av 20 frön — synlig nackdel. Spegel ok.
- **d_tak:** 0 av 20 frön — synlig nackdel. Spegel ok.
