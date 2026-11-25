# hcsim: simulering på Hell Creek-kartans rutor (S5a + S5b)

Steg S5 (2026-11-21). Alt 1196 (crate), 1197 (tal.tsv), 1199 (kartan skrivskyddad),
1198 (populationssvep med ärftligt märke), 1200–1202 (hjärnprovet), 1207–1209
(diagnosen med mätfält), 1213 (avläsarprovet), 1221/1220/1224 (ledprovet),
1228/1229/1230 (rangprovet med trängselmätning och trängselorakel),
1246/1245 (turordningsprovet med omläsning av rangloggen),
1251/1252/1253 (strykprovet på värdets signalväg med delning och bokföring),
1302 (sparat läge: `kor --spara` / `fortsatt --fran`),
1462 (kopieringsprovet: tre kvadratfel mot `v_lar` på fröna 241–260),
1463 (ärvd utforskning: gen ε och `lar_utforsk` på fröna 261–280).
Regel 10 (läst/simulering), 11 (Rust), 14 (hjärnan styr). `n10/varld/` rörs inte.

## Vad den gör

`hcsim kor` bygger en värld på `hckarta::karta_full`: växter per ruta efter klimatet, och
Edmontosaurus som individer med ärvd JEPA-hjärna. Djuren betar, flyttar till grannrutor,
föder och dör. Ingenting utom hjärnan och kroppen styr beteendet.

`hcsim svep` kör populationssvepet Alt 1198: efter 30 års inkörning märks hälften av de
levande (egen slumpgenerator ur fröet). Positivarmen ger märkta 0,9 · KOST_STEG;
nollarmen ger ingen fördel. Tre kartstorlekar × två armar × 20 frön. `--torr` kör 2 frön
på 16×12 och märker varje JSON-rad `TORRKÖRNING`.

`hcsim hjarnprov` kör hjärnprovet (Alt 1200 med 1201 och 1202) på 64×48 med 20 frön per
arm: frusen/lärande/slump/tabell/allvetande/rabatt som enda skillnad mellan märkt och
omärkt grupp (samma hjärnpris). `--torr` = 2 frön, 16×12. Gränser ur
`n10/docs/2026-11-21-lasunderlag-hjarnprov.md`.

`hcsim diagnos` kör diagnosen (Alt 1207 med 1208 och 1209) på 64×48 med 20 **nya** frön
(101–120): mätfält per förmåga, elva armar med speglar, plus förmågorna `vaxt_per_djur`,
`allvetande_slump(p)`, `lar_slump(p)` och `tabell_utan_slump`. `--torr` = 2 frön, 16×12.
Gränser och H1–H3 ur `n10/docs/2026-11-21-lasunderlag-diagnos1207.md`.

`hcsim avlasare` kör avläsarprovet (Alt 1213 med 1218, 1215 och 1214) på 64×48 med 20
frön (121–140): förmågorna `lar_lott`, `lar_intagslasare` och `orakel_forutsagare`, tio
armar med speglar, fel per handling och H4/H4b/H6/H5. `--torr` = 2 frön, 16×12. Gränser
ur `n10/docs/2026-11-21-lasunderlag-avlasare1213.md`.

`hcsim ledprov` kör ledprovet (Alt 1221 med K Alt 1220 och känslighetsprovet Alt 1224)
på 64×48 med 20 frön (141–160): nio armar med speglar och återväxtfaktor per arm, plus
loggkörningen `r_lar` (båda `lar`). R² för G1/G2/G3 räknas ur loggen (anpassning 141–150,
prövning 151–160). Binären `ledprov_r2` läser loggen ensam. `--torr` = 2 frön, 16×12.
Gränser ur `n10/docs/2026-11-21-lasunderlag-ledprov1221.md`.

`hcsim rang` kör rangprovet (Alt 1228 med 1229 och 1230) på 64×48 med 20 frön (161–180):
tre armar (`t_tab`, `t_ork`, `t_to`) med speglar, förmågan `trangsel_orakel`, trängselmätning
(medätare efter alla flyttar, andel knapp, intag), plus loggkörningen `r_lar` med alla 9
hjärnvärden. Binären `rang_r` anpassar G1 på ledprovets logg (141–150, bara läsas) och
räknar träff + Spearman. `--torr` = 2 frön, 16×12. Gränser ur
`n10/docs/2026-11-21-lasunderlag-rang1228.md`.

`hcsim tur` kör turordningsprovet (Alt 1246 med omläsningen Alt 1245) på 64×48 med 20 frön
(181–200): sex armar (`u_tab`, `u_to`, `u_ork`, `u_lar` med turordning; `s_tab`, `s_to` med
samtidig klocka) med speglar och samma trängselmätning som rangprovet. Läget `turordning`
(av som standard) tar djuren ett i taget ur en egen slumpström och betar direkt efter
flytten. Binären `omlas_r` läser bara rangloggen och räknar flyttträff/Spearman, andel där
G1 rankar stå kvar högst, och intag minus G1-medel. `--torr` = 2 frön, 16×12. Gränser ur
`n10/docs/2026-11-21-lasunderlag-tur1246.md`.

`hcsim signal` kör strykprovet på värdets signalväg (Alt 1251 med delningen Alt 1252 och
bokföringen Alt 1253): sex armar (`a_to`, `b_to`, `a_tab`, `b_tab`, `u_to`, `s_to`) med
speglar på fröna 181–200, plus loggkörningen `v_lar` (fröna 201–220). Lägena `flytt_i_tur`
(a) och `bete_i_tur` (b) är av som standard; `turordning` oförändrad. Binären `signal_r`
läser loggen och räknar mått 1–5. `--torr` = 2 frön, 16×12. Gränser ur
`n10/docs/2026-11-21-lasunderlag-v1251.md`.

Alt 1423–1425 (rättat instrument): `ingrepp_traff` tar en avläsare som argument (`lar`,
positiv, negativ, trängsel); kvoten räknas över riktningarna 1–8 (handling 0 för sig).
`hcsim hjsignal --omrakna --logg …` räknar om kvoter och Alt 1424:s sex mått på
`v1251-logg.tsv` (sökning, inte bindande) och skriver `hjsignal1423-*`. R² för riktning ur
`z` räknas per djur (medel över riktningarna, sedan över djuren); oviktat och med raderantal
viktat medel, nollkontroll (y blandad inom djuret, frö `fro ^ R2_NOLL_FRO_XOR`), gemensam
anpassning, misslyckade anpassningar och rader per djur (min/median/max) redovisas. Alt 1425:
`hcsim signal --fron 221-240 --bara v_lar` (full) eller med `--torr` (2 frön, TORRKÖRNING);
körs först efter granskning. Gränser ur `n10/docs/2026-11-22-lasunderlag-hjsignal1423.md`.

`hcsim kopier` kör kopieringsprovet (Alt 1462) på strykprovets `v_lar`-värld (64×48,
samtidig klocka, båda `lar`) med fröna 241–260: tre kvadratfel per `lar`-djursteg
(förutsägare, kopiering, fast del) före viktuppdateringen. `--torr` = 2 frön, 16×12,
märkt TORRKÖRNING. Gränser ur `n10/docs/2026-11-22-lasunderlag-kopier1462.md`.

`hcsim utforsk` kör Alt 1463 (ärvd utforskning) på 64×48 med spegel, fröna 261–280:
förmågan `lar_utforsk` (ärvt ε), armarna `e_lar` / `e_fast` / `e_tab`, måtten aldrig
valda av hjärnan, kopieringskvot per grupp och ε vid slutet. `--torr` = 2 frön, 16×12,
märkt TORRKÖRNING. Gränser ur `n10/docs/2026-11-22-lasunderlag-utforsk1463.md`.

## Taget ur varld (och vad som ändrats)

| Del | Källa | Ändring i hcsim |
|-----|-------|-----------------|
| Klimat `Klimat::ny`, `fas`, `g`, `l`, `vat`, `ft` | `n10/varld/klimat.rs` | Egna tal via `tal.rs`; ingen `K`-karta |
| Växttillväxt | `flockdal.rs` `vaxer_med` ~121 | Tak per ruta = `VMAX/PLATT_KM2 · landyta · vaxt`; M = 0 → `fr = 1−FRO`; ingen tillväxt i hav/flod/sjö; ledprovets `atervaxt` multiplicerar `R_V` och `SOL` (1 = oförändrad) |
| Bete | `flockdal.rs` `betas`/`bete5` ~136–249 | Samma delning; `vill = AT_DINO·V/VMAX` (skalat absolut) |
| Födsel/död | `kamp.rs` `fortplantning8` + `dal_halv2` | Hona + vuxen hane i egen/grannruta; `E_DELA/2`; död vid `e≤0` eller `age≥LIV`; inget NMAX |
| Fader | `didel.rs` / `fortplantning8` | Lottad bland vuxna hanar i 9-rutan |
| Hjärna | `n10/varld/hjarna.rs` | `A_N=9`, `S_N=19`; egen Rng; OPS omräknad |
| Räknestegspris | `didel.rs`: `pris = 0,01·kost/OPS` | `kost = KOST_STEG`; totalt `0,01·kost` |
| Rörelse | ny (S5b) | 9 handlingar varje steg; hav stängt; flod/sjö/träsk extra energi (simulering) |
| Ärftligt märke | Alt 1198 | `markt: bool`; ungen ärver modern; märkning med egen Rng (frö XOR); positivarmen rabatterar bara grundkostnaden |
| Förmågor | Alt 1200–1202, 1207–1209, 1213, 1230 | `formaga` per djur; före märkning alla `lar`; ungar ärver generna (inte inlärda vikter); tabell/slump/allvetande/nya tak-/avläsar-/trängselförmågor betalar samma hjärnpris; intagsläsaren ärvs inte; `trangsel_orakel` värderar `v_andel/(n+1)` ur sinnena |
| Mätfält | Alt 1207, 1213–1214, 1229 | läser bara (handlingar, entropi, intag, död, trängsel, medätare efter flyttar, andel knapp, fel per handling, v-mot-läsare); samma frö med/utan ger samma djur_summa |
| Ledlogg | Alt 1221, 1228 | läser bara (s, a, hjärnvärde(n), intag före/efter, djur_id); samma frö med/utan ger samma djur_summa |
| Turordning | Alt 1246 | läge av som standard; egen slumpström (frö + tillägg); beta direkt; rör inte `lar`/`v`/lika-regeln |
| Flytt i tur / bete i tur | Alt 1252 | lägen a/b av som standard; `turordning` oförändrad |
| Signallogg / ingrepp | Alt 1251 | läser bara (s, z, värde, läge/fast); samma frö med/utan ger samma djur_summa |
| Bokföring | Alt 1253 | läser bara (födslar/döda/intag per 1 000 steg) |

## Kartan (Alt 1199)

hcsim läser kartan och skriver aldrig i den. `hckarta::kontrollsumma` före och efter en körning
ska vara lika (prov `kartan_andras_inte`).

## Tal (Alt 1197)

`tal.tsv`: en rad per konstant (`namn`, `värde`, `enhet`, `marke`, `källa`, `anm`).
Densitetstal räknas per km² med faktorn `PLATT_KM2` (simulering: plättens yta saknar läst källa)
gånger rutans landyta (`ruta_m² · landandel`). Nya svep-tal: `MARKT_RABATT`, `MARKT_INKOR_AR`,
`MARKT_EFTER_AR`. Hjärnprov: `TABELL_VIKT`, `TABELL_SLUMP_P`, `SPEGEL_TOL` (simulering).
Avläsarprov: `LASARE_ETA` (simulering). Återväxtfaktorn i ledprovet är per arm (inte
världskonstant); 1 ger exakt samma tillväxt som i dag.

## Kör

```
cd n10/hcsim
CARGO_TARGET_DIR=/home/hakan/sandbox/pi-hcsim10/n10/hcsim/target nice -n 19 ~/.cargo/bin/cargo build -j 4 --release
./target/release/hcsim kor --lat0 47.0 --lon0 -108.0 --b 64 --h 48 --ruta-m 2000 --t 500000 --fro 1 --ar 50 --n0 400
./target/release/hcsim kor … --spara lage.hcsim --spara-var-ar 5
./target/release/hcsim fortsatt --fran lage.hcsim --ar 20
./target/release/hcsim svep --torr
./target/release/hcsim svep
./target/release/hcsim hjarnprov --torr
./target/release/hcsim hjarnprov
./target/release/hcsim diagnos --torr
./target/release/hcsim diagnos
./target/release/hcsim avlasare --torr
./target/release/hcsim avlasare
./target/release/hcsim ledprov --torr
./target/release/hcsim ledprov
./target/release/ledprov_r2 --logg ledprov1221-logg.tsv
./target/release/hcsim rang --torr
./target/release/hcsim rang
./target/release/rang_r --led /home/hakan/sandbox/pi/work/hcsim/ledprov1221-logg.tsv --rang rang1228-logg.tsv
./target/release/hcsim tur --torr
./target/release/hcsim tur
./target/release/omlas_r --led /home/hakan/sandbox/pi/work/hcsim/ledprov1221-logg.tsv --rang /home/hakan/sandbox/pi/work/hcsim/rang1228-logg.tsv
./target/release/hcsim signal --torr
./target/release/hcsim signal
./target/release/signal_r --logg v1251-logg.tsv
./target/release/hcsim hjsignal --omrakna --logg /home/hakan/sandbox/pi/work/hcsim/v1251-logg.tsv
./target/release/hcsim signal --fron 221-240 --bara v_lar --torr
./target/release/hcsim kopier --torr
./target/release/hcsim kopier
./target/release/hcsim utforsk --torr
./target/release/hcsim utforsk
```

`--spara FIL` skriver hela `Sim` (serde-derive, binärt med `f64::to_bits`), versionsnummer och
kontrollsumma; säker skrivning med `.tmp` / `.forra`. `fortsatt --fran FIL --ar M` bygger kartan
ur parametrarna, kräver samma kartsumma och kör M år till. Utan de nya valen är `kor` oförändrad.

Utskrift var 10:e år, sedan en JSON-rad med sammanfattning. Svepet skriver
`svep1198-korningar.jsonl` och `svep1198-sammanfattning.md`. Hjärnprovet skriver
`hjarnprov-korningar.jsonl` och `hjarnprov-sammanfattning.md`. Diagnosen skriver
`diagnos1207-korningar.jsonl` och `diagnos1207-sammanfattning.md`. Avläsarprovet
skriver `avlasare1213-korningar.jsonl` och `avlasare1213-sammanfattning.md`.
Ledprovet skriver `ledprov1221-korningar.jsonl`, `ledprov1221-logg.tsv` och
`ledprov1221-sammanfattning.md`. Rangprovet skriver `rang1228-korningar.jsonl`,
`rang1228-logg.tsv` och `rang1228-sammanfattning.md`. Turordningsprovet skriver
`tur1246-korningar.jsonl`, `tur1246-omlas.tsv` och `tur1246-sammanfattning.md`.
Signalprovet skriver `v1251-korningar.jsonl`, `v1251-logg.tsv`, `v1251-bokforing.tsv`,
`v1251-signal.tsv` och `v1251-sammanfattning.md`. Kopieringsprovet skriver
`kopier1462-korningar.jsonl`, `kopier1462-tabell.tsv` och `kopier1462-sammanfattning.md`.
Utforskprovet skriver `utforsk1463-korningar.jsonl`, `utforsk1463-tabell.tsv` och
`utforsk1463-sammanfattning.md`.

## Prov

1. `samma_fro_samma_summa`
2. `kartan_andras_inte`
3. `inget_djur_i_havet`
4. `bara_hav_dor_ut`
5. `vaxt_noll_dor_ut`
6. `tal_tsv_tacker_konstanterna`
7. `hjarnan_lar_sig`
8. `market_arvs_fran_modern`
9. `rabatten_bara_for_markta`
10. `nollmarket_andrar_inte_varlden`
11. `halften_markas`
12. `frusna_vikter_star_still`
13. `larande_vikter_andras`
14. `samma_hjarnpris`
15. `ungen_arver_generna_inte_inlarda`
16. `spegeln_byter_bara_formagan`
17. `tabellen_provar_ibland`
18. `matningen_andrar_inget`
19. `slumpens_handlingar_jamna`
20. `tom_matning_stoppar`
21. `vaxt_per_djur_valjer_ratt`
22. `allvetande_slump_p_noll_ar_allvetande`
23. `tabell_utan_slump_provar_aldrig` (Alt 1218: genom `Sim`, djuret lever)
24. `lar_lott_lottar`
25. `lasaren_lar_sig`
26. `orakel_ser_malrutan`
27. `r2_pa_kant_samband` (kontroll 1)
28. `r2_omkastade_mal` (kontroll 2)
29. `r2_skilda_fron_vagrar_overlap` (kontroll 3)
30. `tom_logg_stoppar` / `loggen_andrar_inget` (kontroll 4)
31. `atervaxt_faktorn_nar_simuleringen` (kontroll 5; kräver faktorn i både R_V och SOL; mutanten "bara R_V" blir röd)
32. `g2_indata_ar_hjarnvarde` (lucka: mutanten "G2 med intaget som indata" blir röd)
33. `r_pa_kanda_djur` (rang kontroll 1)
34. `r_skalan_per_djur` (rang kontroll 2)
35. `medatare_efter_flyttar` (rang kontroll 3)
36. `trangsel_orakel_valjer_ratt` (rang kontroll 4)
37. `matfalt_trangsel_andrar_inget` (rang kontroll 6)
38. `turordning_av_som_idag` (tur kontroll 1)
39. `turordning_ensamt_djur` (tur kontroll 2)
40. `turordning_lottas` (tur kontroll 3)
41. `turordning_fordelning_2_och_1` (tur kontroll 4)
42. `turordning_betet_direkt` (tur kontroll 5)
43. `forskjutna_varden_foljer_valet` (tur kontroll 6)
44. `omlas_1245_kontroller` (tur kontroll 7)
45. `tur_spegel_och_hjarnpris_kvar` (tur kontroll 8)
46. `v0_andrar_inte_rang` (signal kontroll 1)
47. `mutanter_klassas_ratt` (signal kontroll 2)
48. `ingreppet_laser_bara` (signal kontroll 3)
49. `lage_a_och_b` (signal kontroll 4)
50. `upprepning_u_to_s_to` (signal kontroll 5)
51. `bokforingen_fanger_tappad_fodsel` (signal kontroll 6)
52. `kendalls_w_kontroller` (signal kontroll 7)
53. `signal_spegel_och_hjarnpris_kvar` (signal kontroll 8)
54. `kontroll1`–`kontroll8` i `tests/spara1302.rs` (Alt 1302 sparat läge)
55. `avlasare_ingrepp_traff` / `mutant_hardkodad_positiv_traff_upptacks` (Alt 1423)
55b. `r2_per_djur_olika_vikter` / `r2_per_djur_oberoende` / `r2_misslyckad_singular_raknas` / `r2_nollkontroll_linjar_skillnad` (Alt 1424 R²)
56. `kvot_riktningar_ignorerar_handling_0` (Alt 1423)
57. `lar_ingrepp_samma_som_forr` (Alt 1423)
58. `v_kvot_upprepas_pa_loggen` (Alt 1423)
59. `orsak_matt_pa_konstruerade_rader` (Alt 1424)
60. `kontroll1`–`kontroll5` i `tests/kopier1462.rs` (Alt 1462 kopieringsprov)
