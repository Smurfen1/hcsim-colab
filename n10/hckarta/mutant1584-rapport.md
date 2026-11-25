# Rapport: Alt 1584 (träskmutanter i tidsmodell 2)

Skriven 2026-11-22 av Cursor i arbetsträdet `pi-hckarta10`, gren `hckarta-mutant1584`.
Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartstd2.md` (Alt 1584),
skrivet före bygget. Mätning som visade felet:
`/home/hakan/Projekt/software30/piharness/docs/evolution/kartstd-matning.txt`
(mutant A och B samma tal som utan mutant i tidsmodell 2).

## Git

```
git rev-parse HEAD (kodcommit): 803495e6cfbb19cb5b26f7f8082ae323a8dc99ef
git rev-parse HEAD (bas före uppdraget): 002e9d84fa802112ec85de40bc749087c64951f2
```

`git status --short` före första commit (endast tillåtna filer):

```
 M n10/hckarta/LASMIG.md
 M n10/hckarta/src/flod.rs
 M n10/hckarta/src/lib.rs
?? n10/hckarta/tests/mutant1584.rs
```

## Vad som byggdes

- `lagg_version2_urval` och grenen med `lutning >= 2` i `lagg_foror_som_objekt` går genom
  `trask_val` med en `TraskProvMutant` (inte längre `TraskYta::Narmaste` direkt).
- `karta_for_ar_lutning_prov(..., mutant)` och `karta_for_ar_fyllnad_lutning_prov` — bara proven;
  produktionsvägen (`karta_for_ar_lutning` / `karta_for_ar_fyllnad_lutning`) skickar alltid
  `TraskProvMutant::Ingen`.
- Utan mutant: byte för byte som före (prov 2).
- Inga gränser, tätheter eller standarder ändrade. Johnsons prov 6 (frö 181) i `lutning1485.rs`
  rördes inte. Inga befintliga provfiler ändrade.

## Provutfall

| Prov | Utfall | Anmärkning |
|------|--------|------------|
| 1 mutanter syns i TM2 | grönt | A och B skiljer i 20/20 frön vid t = 500 000, 20 000 och 0 |
| 2 utan mutant oförändrat | grönt | `karta_for_ar_lutning` ≡ prov med `Ingen` (vattentyp, vatten_yta, fara_andel) |
| 3 minst 20 frön / 0 fårrutor | grönt | 1 frö → fel; 0 fårrutor → fel; 20 + ≥1 → ok |

## Tabell: antal Trask-rutor per frö (version 2, riktning 2, lutning 2)

### t = 500 000 (Aldre)

| frö | utan | A | B |
|----:|-----:|--:|--:|
| 1 | 353 | 229 | 706 |
| 2 | 437 | 267 | 750 |
| 3 | 390 | 240 | 685 |
| 4 | 365 | 252 | 717 |
| 5 | 442 | 306 | 793 |
| 6 | 380 | 263 | 726 |
| 7 | 412 | 257 | 746 |
| 8 | 448 | 292 | 764 |
| 9 | 420 | 289 | 799 |
| 10 | 401 | 253 | 671 |
| 11 | 389 | 251 | 687 |
| 12 | 409 | 271 | 710 |
| 13 | 289 | 188 | 578 |
| 14 | 394 | 269 | 768 |
| 15 | 506 | 363 | 797 |
| 16 | 467 | 338 | 758 |
| 17 | 392 | 237 | 676 |
| 18 | 360 | 240 | 622 |
| 19 | 409 | 282 | 728 |
| 20 | 349 | 225 | 636 |

A skiljer i 20/20; B skiljer i 20/20.

### t = 20 000 (Ovre)

| frö | utan | A | B |
|----:|-----:|--:|--:|
| 1 | 164 | 124 | 344 |
| 2 | 203 | 117 | 406 |
| 3 | 202 | 130 | 417 |
| 4 | 141 | 87 | 303 |
| 5 | 149 | 115 | 280 |
| 6 | 198 | 134 | 470 |
| 7 | 226 | 152 | 372 |
| 8 | 182 | 124 | 392 |
| 9 | 103 | 68 | 240 |
| 10 | 194 | 118 | 312 |
| 11 | 98 | 65 | 298 |
| 12 | 221 | 130 | 412 |
| 13 | 199 | 141 | 372 |
| 14 | 246 | 176 | 586 |
| 15 | 179 | 117 | 359 |
| 16 | 111 | 89 | 322 |
| 17 | 135 | 80 | 203 |
| 18 | 217 | 143 | 443 |
| 19 | 134 | 109 | 317 |
| 20 | 219 | 170 | 512 |

A skiljer i 20/20; B skiljer i 20/20.

### t = 0 (Gransen)

| frö | utan | A | B |
|----:|-----:|--:|--:|
| 1 | 155 | 113 | 329 |
| 2 | 203 | 117 | 406 |
| 3 | 200 | 129 | 418 |
| 4 | 135 | 86 | 297 |
| 5 | 148 | 113 | 279 |
| 6 | 190 | 127 | 446 |
| 7 | 217 | 141 | 363 |
| 8 | 176 | 119 | 386 |
| 9 | 124 | 83 | 260 |
| 10 | 185 | 112 | 303 |
| 11 | 91 | 57 | 291 |
| 12 | 221 | 130 | 412 |
| 13 | 196 | 141 | 369 |
| 14 | 243 | 171 | 601 |
| 15 | 170 | 111 | 350 |
| 16 | 100 | 81 | 311 |
| 17 | 164 | 108 | 233 |
| 18 | 143 | 79 | 266 |
| 19 | 134 | 109 | 317 |
| 20 | 209 | 161 | 502 |

A skiljer i 20/20; B skiljer i 20/20.

## Röda proven (gammal kod)

Kopia där `lagg_version2_urval` satte `TraskYta::Narmaste`/`Lagsta` direkt och ignorerade
`mutant` (samma som före Alt 1584). Prov 1 vid t = 500 000:

```
=== prov1 t=500000: Trask per frö (utan / A / B) ===
  fro=1: utan=353 A=353 B=353
  … (alla 20 lika)
  fro=20: utan=349 A=349 B=349
=== prov1 t=500000: A skiljer i 0/20; B skiljer i 0/20 ===
mutant A skiljer bara i 0 av 20 frön vid t=500000 (mål ≥ 18)
test prov1_mutanter_syns_i_tidsmodell2 ... FAILED
```

Överensstämmer med mätningen i `kartstd-matning.txt`: A och B samma som utan mutant i
tidsmodell 2 innan vägarna gick genom `trask_val`.

## cargo test

hckarta (`CARGO_TARGET_DIR=…/n10/hckarta/target`, `--release`):

- `mutant1584`: **3 passed**, 0 failed.
- `lutning1485`: 15 passed (inkl. Johnsons prov 6, frö 181).
- Övriga (fara/bredd/tid/vall/vaxt/riktning/lopp/fyll/s1–s4/granser): alla gröna utan
  ändring av anrop.

hcsim (`CARGO_TARGET_DIR=…/n10/hcsim/target`, `--release`): lib 54 passed; spara1302
13 passed, 2 ignored; övriga bin/tester gröna.

## Avvikelser

Inga. Gränser och standarder oförändrade. Mutant A ger färre Trask (närmaste := lägsta),
mutant B fler (dubblat avstånd) — samma riktning som i tidsmodell 1 (Alt 1533).
