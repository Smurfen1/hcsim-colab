# hckarta: rutnät över Hell Creek vid tiden t

Steg S4 (2026-11-21). Regel 10 (märket läst/simulering), 11 (Rust) och 14. Underlag:
`n10/docs/2026-11-20-lasunderlag-hckarta.md` avsnitt 1–3; `n10/dinobiom/HELL-CREEK-PERIODER.md`
avsnitt 2–3. Beslut: Alt 1150, 1153, 1156, 1165–1168, 1180–1182, 1188; flodnät (Alt 1167) i stället
för ringen av dalar. S1–S3 ihopslagna (sandbox/pi 179056e).

## Vad den gör

`karta(källa, område, t)` ger ett rutnät över ett område i dagens koordinater. Varje rutas mittpunkt
flyttas till sitt läge för 66 miljoner år sedan med `paleokarta::Rotationer::flytta(101, …, 66.0)`
(platta 101, Nordamerika). Höjden läses bilinjärt i PaleoDEM `Map16_065Ma.csv`. I S1–S4 används 66 Ma
och Map16 för alla t; t lagras. Fel vid saknad rotation ger `Err`, aldrig ett tyst 0.

`karta_med(källa, tidtabell, område, t)` gör samma sak men sätter periodlagret ur tidtabellen
(alla rutor samma period, märket Last). Utan tidtabell är perioden `Aldre` med märket Simulering.

`karta_full(…, fro)` = `karta_med` plus `lagg_flodnat`: flodnät ur fröet och periodens regler
(alltid **flodversion 1**, **flodriktning 1** och **lutning 1**). `karta_full_version(…, fro,
flodversion, flodriktning)` tar version 1 eller 2 och riktning 1 eller 2 (lutning 1);
`karta_full_version_lutning` tar även lutning 1 eller 2. Version 2 returnerar även
`FlodAndelar` (ligger utanför `Karta` så hcsim:s konstruktorer bryts inte).
`karta_for_ar(…, fro, flodversion, flodriktning)` (Alt 1401/1395/1494, `--tidsmodell 2`) bygger fårorna
som objekt med historik från basen och loppbyten; `karta_for_ar_lutning` tar även lutning;
`karta_for_ar_lutning_prov` tar dessutom en `TraskProvMutant` (Alt 1584; bara proven — produktion
skickar alltid `Ingen` via `karta_for_ar_lutning`); se avsnittet nedan.
`nasta_andring(tt, t, fro)` ger närmaste t' &lt; t där perioden byts, ett lopp byts **eller** en
fyllning blir klar. `loppbyten(…)` listar bytena (år, fåra, punkt).

`skriv_3d(karta, mapp)` / `las_3d(mapp)` skriver och läser filerna till 3D-bilden (samma `karta` som
simuleringen). `skriv_3d_med` / `las_3d_med` tar även andelslagren vid flodversion 2. Kontrollsumman i
`karta.json` ska stämma med den lästa kartan (version 2: summan tar med andelslagren och, om
`foror.tsv` finns, bredderna; vid Alt 1371 även vallagren och `vall_m`; vid Alt 1433 även
`vaxt_tathet` och gruppernas höjd/artantal). En mapp från Alt 1343 (utan `foror.tsv`), Alt 1386
(utan vallfiler) eller Alt 1371 (utan växtgrupper) går fortfarande att läsa. Det gamla lagret `vaxt`
ändras inte.

Lager:

| Lager | Innehåll | Märke |
|-------|----------|-------|
| `hojd` | m över havsytan (PaleoDEM; i fönstret **bilinjär** interpolation mellan gradpunkter); med `--lutning 2` ersatt plan | Last / Simulering (lutning 2) |
| `vatten_yta` | utan flod: 0 där höjd &lt; 0, annars = höjden; med flod: sänkt i fåror/träsk/sjöar | Last / Simulering (efter flod) |
| `vattentyp` | utan flod: Hav/Land; med flod: Flodfara, Trask, Sjo tillagda | Last / Simulering (efter flod) |
| `mark` | `[0.25; 4]` (sand, silt, lera, torv) | Simulering |
| `vaxt` | `0.5` | Simulering |
| `period` | ur tidtabellen (`Aldre`/`Ovre`/`Gransen`) vid `karta_med`; annars `Aldre` | Last / Simulering |
| `fara_andel` | (bara flodversion 2) andel 0..1 av rutan som fåra; fil `fara_andel.f32` | Simulering |
| `balte_andel` | (bara flodversion 2) andel 0..0,85; fil `balte_andel.f32`; ≥ `fara_andel` | Simulering |
| `vall_andel` | (bara flodversion 2, Alt 1371) andel 0..1 − balte; fil `vall_andel.f32` | Simulering |
| `vall_hojd` | (bara flodversion 2, Alt 1371) 2,0 m där vall &gt; 0, annars 0; fil `vall_hojd.f32` | Simulering |
| `vaxt_tathet` | (bara flodversion 2, Alt 1433) sex andelar 0..1 per ruta; fil `vaxt_tathet.f32` | Simulering |
| `vaxtgrupper` | (bara flodversion 2, Alt 1433) namn, höjd, märke, antal arter; fil `vaxtgrupper.json` | Simulering |
| `foror` | (bara flodversion 2, Alt 1386+) en rad per fåra: `fara`, `bredd_m`, `balte_m`, `rutor` [, `vall_m`]; fil `foror.tsv` | Simulering |

## Flodnätet (S4)

Källorna (lästa): Roberts m.fl. 2026 — anastomosed fluvial morphology i den nedre delen, pollen med
fluvial/paludal/lacustrine; DePalma m.fl. 2019 — meandering rivers flowing east into the WIS.
Riktningen österut är läst. Fårornas antal, bredd, träskbälte och sjötäthet är **egna val** (simulering),
byggda ur PaleoDEM:s lutning och ett frö (SplitMix64 i `flod.rs`).

Algoritm (riktning 1, standard):

1. Starta `foror` fåror vid västra kanten (jämnt fördelade i nordsyd med frö-jitter).
2. Varje steg: bland de tre östra grannarna (NO, Ö, SO) välj den lägsta höjden; vid skillnad ≤ 0,5 m
   (eget val) lottas bland de nästan lika. Frö-brus ±2 m och +8 m straff på redan Flodfara (egna val)
   håller isär flera fåror på den grova PaleoDEM-ytan.
3. Fåran sätts till `Flodfara`, `vatten_yta = höjd − 2 m`, utvidgad `bredd_rutor` i nordsyd.
4. Rutor inom Chebyshev-avstånd `trask_avstand` från en fåra och med höjd &lt; fåran + 1 m → `Trask`.
5. Sjöar: `sjoar_per_100_rutor` per 100 kartrutor, bland de lägsta rutorna nära fårorna → `Sjo`.
6. Hav (höjd &lt; 0) ändras aldrig. Efteråt: `vattentyp` och `vatten_yta` märks Simulering.

## Flodriktning 2 (Alt 1372 / 1472)

Bakom `--flodriktning 2` (standard 1 = oförändrat österut, byte för byte). Hasler och Renne 2025 [A]
(Fort Peck): före gränsen "a near-uniform distribution dominated by a NW-SE axis, which likely
reflects southeastward drainage…". Butler 1980 [A] (Glendive): "Paleocurrents … trend mostly east
and southeast". Att samma riktning gäller i **Ovre** är **eget val**.

1. **Stegen:** öst (1,0), sydost (1,1), söder (0,1); ingen bakåt. Val som i riktning 1 (lägst höjd,
   brus ±2 m, +8 m på redan Flodfara) plus **riktningsstraff k meter** på söderstegen.
2. **k:** minsta av 0, 2, 4, 8, 16, 32 där medelriktningen frö 1–50 Aldre ∈ **125–140°** (0° = norr,
   90° = öst; mätt första→sista ruta). Intervallet och listan = **eget val**. Mätning: alla k ger
   medel 132,0° (söder väljs bara när Ö/SO saknas); därför `FLODRIKTNING_STRAFF_M` = **0**.
3. **Starter:** jämnt längs västra kanten söder→norr och vidare längs **norra** kanten till östra
   änden (samma antal fåror, jitter ±1). **Eget val** (Alt 1472).
4. **Slut:** när nästa steg skulle gå ut (öst/syd-kant) eller i hav.
5. **Längd:** `L = ruta_m · √(di² + dj²)` till nästa ruta (sista: `L = ruta_m`); andelar med detta L.
6. `karta_full` (hcsim) ger alltid riktning 1.

## Lutning 2 (Alt 1485 / 1490 / 1488)

Bakom `--lutning 2` (standard 1 = oförändrad höjd och vattenyta, byte för byte). PaleoDEM:s höjd i
fönstret är **bilinjär** interpolation mellan gradpunkter (en punkt per grad). Lutning 2 **ersätter
fönstrets plan**: det anpassade planet (minsta kvadrat) dras bort och ett plan som faller
**0,30 m/km** (Butler 1980 [A], räknat med Schumms ekvation) mot **135°** (**eget val**; Hasler och
Renne NW–SE-axel) läggs in, med samma medelhöjd över landrutorna. Havsrutor orörda; landruta under 0
→ fel (ingen tyst klämning). Att **fårans lutning används för landet** är **eget val**.

Vattenytan längs fåran: `yta = min(förra rutans yta, höjd − 2 m)` (första: höjd − 2 m); träsk mot
ytan + 1 m i stället för fårans höjd + 1 m. Med `--lutning 2` tar träskregeln ytan från **närmaste**
mittlinjeruta (Chebyshev; vid lika avstånd den lägsta ytan) i stället för den lägsta ytan inom
`trask_avstand` (Alt 1501). Utan `--lutning 2` är träskregeln oförändrad. Gäller båda
flodriktningarna och båda tidsmodellerna. Varken `--flodriktning 2` eller `--lutning 2` är standard
i detta bygge (Alt 1490); hcsim byter först efter BRAIN. `karta_full` ger alltid lutning 1.

Prov 5a under `--lutning 2` (Alt 1533): träsk per fårruta **1,35–2,10** (riktning 2) och **0,16–0,40** (riktning 1) = **eget val** utan källa (skydd mot oavsiktlig regeländring, inte bevis för realism).

Träskmutanter A/B (Alt 1533 / 1584): går genom `trask_val` även i tidsmodell 2 (`lagg_version2_urval`
och grenen med lutning ≥ 2 i `lagg_foror_som_objekt`). Utan mutant är beteendet oförändrat.

Egna tal per period (Gransen rättad Alt 1405); `flodregler` oförändrad:

| Period | foror | bredd_rutor | trask_avstand | sjoar_per_100_rutor | motiv |
|--------|------:|------------:|--------------:|--------------------:|-------|
| Aldre | 5 | 2 | 3 | 2,0 | anastomosed, flera fåror (Roberts) |
| Ovre | 2 | 2 | 3 | 1,5 | meandrande, färre (DePalma) |
| Gransen | 2 | 2 | 3 | 2,0 | bredd minskade (Butler 1980 [A]); mer stående vatten (Hasler och Renne 2025 [A]); talen 3 och 2,0 = **eget val** |

**Träskbälte 2 (Alt 1643):** i **tidsmodell 2 med lutning ≥ 2** sätts `trask_avstand` till **2** i
Aldre och Ovre (Gransen behåller 3). Tidsmodell 1, lutning 1 och `flodregler` / `karta_full` ändras
inte byte för byte. Talet 2 = **eget val**: ingen läst källa ger träskets bredd. Riktningen har läst
stöd: USGS PP 414-F (doi 10.3133/pp414f): "Lignite and lignitic beds are present although very sparse
and discontinuous in the upper part of the formation"; USGS OF 81-786 (doi 10.3133/ofr81786): "Coals
in the formation rarely exceed 3 feet in thickness". Kartans `Trask` är blöt mark på slätten, inte
kolträsk.

## Fåror som objekt / tidsmodell 2 (Alt 1401 + Alt 1395)

Bakom `--tidsmodell 2` (standard 1 = dagens `karta_full` / `karta_full_version`). `karta_full` och
tidsmodell 1 ändras **inte** (byte för byte). Bara tidsmodell 2 får loppbyten.

1. **Historiken börjar** vid tidtabellens lästa rad `basen` (1 081 000 år före gränsen) med Aldres
   fåror (`kor_mittlinjer`). Att historiken börjar vid basen = **eget val** (Alt 1395).
2. **Vilka som lever:** i Aldre alla; från Ovre (t ≤ 290 000) **två** ur egen SplitMix64-ström
   (`fro ^ 0x4C45_5646_4152_4F52`); Gransen behåller samma två. Urvalet och samtidig död =
   **eget val** (Alt 1401). Döda fåror byter inte lopp.
3. **Loppbyten (Alt 1395):** varje levande fåra får byten med mellanrum dragna jämnt i
   **82 000–152 000** år ur delström `fro ^ 0x4C4F_5050_4259_5445` (ASCII "LOPPBYTE") per fåra.
   Takten är **räknad** ur Butler 1980 [A] ("Upward-fining intervals, each about 6 m thick") och
   Yamamura 2024 [A] ("the 11m sequence may represent 151,000 to 278,000 years"):
   6 m · 151 000–278 000 / 11 m. Ingen källa ger takten direkt.
4. **Var:** bytet i en punkt dragen jämnt bland fårans rutor från **20 % till 80 %** av längden
   (**eget val**). Uppströms behålls.
5. **Nytt lopp:** samma stegregler (riktning 1 eller 2); rutor på annat lopp (levande eller
   övergivet) får **bonus 8 m** (lägre kostnad); egen gammal nedström **straff +8 m**. Talet 8 m =
   **eget val** (samma storlek som dagens Flodfara-straff). Butler 1980 [A]: "directed toward
   pre-existing meander belts".
5b. **Stigningsgräns (Alt 1604):** vid **lutning 2** får ett nytt lopp efter loppbyte inte ha en
   ruta mer än **`LOPP_MAX_STIGNING_M` = 5 m** över den lägsta höjden uppströms på samma lopp
   (från fårans start genom bytespunkten). När ett steg skulle bryta gränsen väljs bland tillåtna
   steg det som stiger minst; finns inget sådant sker bytet inte. Lutning 1, första loppen och
   tidsmodell 1 orörda. Slingerland och Smith 2004 [A] (avulsion); regeln vilar på att vatten inte
   rinner uppför. Talet 5 m = **eget val**.
5c. **Möte (Alt 1605):** i **produktionen** vid **lutning ≥ 2** i tidsmodell 2 (Alt 1643/1644). När
   ett nytt lopp efter loppbyte når en ruta i ett **annat levande** lopp slutar det där och flyter
   ihop: nedströms gäller det andra loppet; vattenytan i mötesrutan är den lägre av de två.
   Fallande yta beräknas **per lopp** och min tas bara i delade rutor — den lägre ytan propagerar
   inte som `prev` in i det andra loppet. Lutning 1 och tidsmodell 1 behåller den gamla itererade
   delningen. Övergivna lopp ger fortfarande bonus men stoppar inte. `LoppProvMutant::Ingen` =
   mötet på; mutanten `MotetAvslaget` stänger av mötet (rött i korsmåttet).
6. **Övergiven nedström / död fåra:** → `Trask`, `vatten_yta = höjd` i **`FYLLNADSTID_AR` = 1000** år
   (t ∈ (t_övergiven − 1000, t_övergiven]), därefter `Land` med `vatten_yta = höjd` utom där ett
   levande lopp går eller träsk/sjö av en annan regel (**eget val**, Alt 1494; Butler 1980 [A]:
   "gradual in-filling … sand, and then silt, clay, and peat"). Gäller loppbyten och Alt 1401:s
   tre fåror som dör vid 290 000. Fyllda delar ligger kvar i listan över tidigare lopp (bonus 8 m;
   Alt 1496 ingår **inte**).
7. Träsk, sjöar, bälte, vall och växtgrupper räknas om från de **levande** loppen. Bredd `w_f`
   följer fåran (ursprungligt nummer).
8. `nasta_andring(tt, t, fro)` = närmaste t' &lt; t med periodbyte, loppbyte **eller** fyllning klar
   (t_övergiven − 1000). `loppbyten(…)` listar (år, fåra, punkt).

## Flodversion 2 (Alt 1343 + Alt 1344 + Alt 1386 + Alt 1371 + Alt 1433)

Bakom `--flodversion 2` (standard är 1). Mittlinjen är exakt version 1:s mittlinje (samma pathfinding
med `bredd_rutor` och +8 m-straffet under körningen); version 2 bygger lagren ur den listan och
behåller **inga** `Flodfara`-rutor. Bredd per fåra (Alt 1386) dras ur en **egen** SplitMix64-ström
(frö `fro ^ 0x4252_4544_4431_3338`); vägsökningens och sjöarnas ström rörs inte.

| Tal | Värde | Märke | Källa / val |
|-----|------:|-------|-------------|
| `FARA_BREDD_MIN_M` | 100 m | Simulering | Butler 1980 [A], lägsta mätta bredd (Huff "100 to 120 m") |
| `FARA_BREDD_MAX_M` | 320 m | Simulering | Hasler och Renne 2025 [A], medel före gränsen 315 m avrundat (w = R / 2,3) |
| fördelning | jämn i logaritmen | Simulering | **eget val**; samma gränser i båda perioderna = **eget val** |
| `BALTE_FAKTOR` | 4 | Simulering | **eget val** inom Butlers 300/130 till 600/110 ≈ 2,3–5,5 |
| `balte_andel`-tak | 0,85 | Simulering | **eget val** (Alt 1344) |
| vatten_yta i mittlinje | höjd − 2 m | Simulering | samma som version 1 (**eget val**) |
| `VALL_FAKTOR` | 4 | Simulering | **eget val** inom Butlers 3–5 (Bacon Creek: "three to five times the channel width") |
| `VALL_MAX_M` | 600 m | Simulering | Butler 1980 [A], "as much as 600 m wide"; att 600 m gäller **båda sidorna tillsammans** = **eget val** |
| `VALL_HOJD_M` | 2,0 m | Simulering | Butler 1980 [A], medelhöjd 2 m (Bacon Creek 1–3 m; Colgate/Little Beaver/Marmarth "about 2 m") |
| övergivna fåror | ingen vall | Simulering | **eget val** (vall försvinner med fåran) |
| genombrott | byggs inte | — | Butler: "probably present, but were not identified" / "a lack of crevasse splay deposits"; ingen storlek |

## Växtgrupper (Alt 1416 / 1433)

Sex grupper i fast ordning. **Ingen källa** ger höjd i meter för Hell Creeks träd eller skogens
täthet; höjderna och tätheterna är **egna val**. Johnson 2002 [A]: "angiosperm-dominated woodland
composed of small- to medium-sized trees"; "a diverse angiosperm-dominated herbaceous vegetation
associated with large paleochannels in the middle of the formation"; "ferns, fern allies, cycads,
ginkgo, and conifers represent &lt;10% of total taxa and specimens". Artkopplingen läses ur
`vaxtgrupper.tsv` (skriven av Claude; ändras inte här).

| nr | grupp | höjd (m) | märke |
|---:|-------|---------:|-------|
| 0 | `lovtrad` | 15,0 | **eget val** ("small- to medium-sized trees", Johnson 2002 [A]) |
| 1 | `barrtrad` | 25,0 | **eget val** (mitt i nu levande Glyptostrobus 20–30 m) |
| 2 | `ort` | 0,5 | **eget val** |
| 3 | `ormbunke` | 1,0 | **eget val** |
| 4 | `kottepalm_ginkgo` | 2,0 | **eget val** |
| 5 | `vattenvaxt` | 0,1 | **eget val** |

Täthet per ruta (andel av rutans yta; alla **egna val** utom örttillägget som bygger på Johnson [A]):

| vattentyp | lovtrad | barrtrad | ort | ormbunke | kottepalm_ginkgo | vattenvaxt |
|-----------|--------:|---------:|----:|---------:|-----------------:|-----------:|
| Land | 0,50 | 0 | 0,10 | 0,02 | 0,01 | 0 |
| Trask | 0 | 0,25 | 0,10 | 0,05 | 0 | 0,10 |
| Sjo | 0 | 0 | 0 | 0 | 0 | 0,30 |
| Hav | 0 | 0 | 0 | 0 | 0 | 0 |

I **Aldre** får en Land-ruta med `balte_andel > 0` eller `vall_andel > 0` örttätheten **0,30** i
stället för 0,10 (Johnson 2002 [A] om örter vid stora fåror i mitten av formationen; att det gäller
just Aldre och +0,20 = **eget val**). Barrträd och lövträd i träsk = **eget val**. I standardvägen (tidsmodell 2 med lutning ≥ 2; `vaxt_tathet_ruta_standard`, `TRASK_TATHET_ALT1694`) gäller Alt 1694 (2026-11-22): lövträd 0,05 och barrträd 0,20 i träsk i stället för 0 och 0,25 (tabellen ovan gäller tidsmodell 1 och lutning 1, oförändrad), summan 0,50 kvar; valt som minsta steg om 0,05 som håller Johnsons kvot under larmgränsen 0,085 (eget val) vid fem tider på fröna 1–20; Johnson 2002 [A]: "angiosperm-dominated woodland"; protokollet `n10/docs/2026-11-22-omrostning-kartstd5.md`). Att Johnsons "&lt;10 %" styr
täthet (inte bara taxa) = **eget val**. I tidsmodell 2 följer tätheten kartan för år t (övergivna
fåror är träsk och får träskets växter). Filerna `vaxt_tathet.f32` (6·b·h f32 LE) och
`vaxtgrupper.json` skrivs bara i flodversion 2.

Formel: fåra f får `w_f = exp(ln 100 + u_f · (ln 320 − ln 100))`, `b_f = 4 · w_f`,
`v_f = min(600, 4 · w_f)`;
`L = ruta_m · √(di² + dj²)` till nästa ruta (sista rutan: `L = ruta_m`; riktning 1 har di = 1 så
√(1 + dj²) som före);
`fara_andel = min(1, w_f · L / ruta_m²)`; `balte_andel = min(0,85, b_f · L / ruta_m²)` och alltid
≥ `fara_andel`; `vall_andel = min(1 − balte_andel, Σ v_f · L / ruta_m²)` (vallen ligger utanför
bältet, Butler). `vall_hojd_m` = 2,0 där `vall_andel > 0`, annars 0; `hojd` ändras inte.
Flera fåror i samma ruta adderas och klipps. Bara **levande** fåror har vall (tidsmodell 2 efter
290 000 år: de två levande). Träsk och sjöar som i version 1 men från mittlinjerutorna. Bältet som
flodbälte längs den slingrande fåran är **inte belagt** som mått i källorna (eget val inom Butlers
intervall).

hcsim anropar `karta_full` (version 1) och får samma slutsummor som före. Med version 2 utan ändring
i hcsim skulle simuleringen varken få flodkostnad eller startspärr i fåran; hur andelen och vallen
ska användas avgör sessionen BRAIN. Inget i `n10/hcsim/` ändras här.

## Tidtabellen

Filen `tidtabell.tsv` (skriven av Claude ur lästa källor; Cursor ändrar den inte) har fem fält per rad
skilda med tab: år före gränsen, händelse, värde, källa, märke. Rader som börjar med `#` hoppas över.
Tom tid = okänd (`None`). Bara rader med märket `läst` och känd tid får styra kartan.

`Tidtabell::period(t)`:

- `Gransen` om `t ≤` tiden för den lästa raden `gransen`
- `Ovre` om `t ≤` tiden för den lästa raden `magnetvandning`
- annars `Aldre`
- `Err` om någon av de två lästa raderna saknas (eller saknar tid)

En tom tabell, eller en tabell utan läst rad med känd tid, ger `Err` vid `las`.

## Rutans mittpunkt (plant närmevärde)

Sydvästra hörnet är `(lat0, lon0)`. Ruta `(i, j)` har i = 0‥b österut, j = 0‥h norrut. Mittpunkten:

- 1° latitud = 111 320 m
- 1° longitud = 111 320 · cos(lat) m, med `lat` = mittpunktens bredgrad

```
lat = lat0 + (j + ½) · ruta_m / 111320
lon = lon0 + (i + ½) · ruta_m / (111320 · cos(lat))
```

## Filerna till 3D-bilden (S3)

Mappen innehåller:

| Fil | Innehåll |
|-----|----------|
| `karta.json` | version 1, område, t, per lager fil/typ/märke, periodnamn, kontrollsumma (decimalsträng), hojd_png-skala |
| `hojd.f32` | f32 LE, b·h, radvis från sydväst |
| `vatten_yta.f32` | f32 LE |
| `vattentyp.u8` | u8 |
| `mark.f32` | 4×f32 per ruta (sand, silt, lera, torv) |
| `vaxt.f32` | f32 LE |
| `period.u8` | u8 |
| `hojd.png` | 16-bitars gråskala; värde = (höjd_m + 1000)·10, avrundat, 0‥65535; egen zlib (store) |
| `strand.json` | strandlinjen i rutor och i meter (`kurvor` / `kurvor_m`) |

`strandlinje(karta)`: marching squares över rutornas mittpunkter (x = i+½, y = j+½) där höjden går genom 0 m.
Tom lista om hela kartan är land eller hela är hav. PNG och strand läses inte tillbaka av `las_3d`.

## Kontrollsumma

`kontrollsumma` är FNV-1a 64 bitar över lagren i ordningen höjd, vatten_yta, vattentyp, mark, växt,
period: alla värden little endian (period och vattentyp som `u8`), sedan märket som `u8`
(`Last = 0`, `Simulering = 1`). Vid flodversion 2 följer `fara_andel` och `balte_andel` (värden + märke)
efter period (`kontrollsumma_med`); vid Alt 1371 (`har_vall`) följer `vall_andel` och `vall_hojd_m`;
om `foror` inte är tom följer varje fåras `bredd_m`, `balte_m` (f64 LE) och `rutor` (u64 LE), och
vid `har_vall` även `vall_m` (f64 LE); vid Alt 1433 (`har_vaxtgrupper`) följer `vaxt_tathet`
(sex f32 per ruta + märke) och varje grupps `hojd_m` (f32 LE) och `arter` (u64 LE). Samma karta ger
samma summa; ändras ett värde eller märke i vilket lager som helst blir summan en annan. `las_3d`
kräver att den beräknade summan är densamma som i `karta.json`.

## Kör

```
cd n10/hckarta
CARGO_TARGET_DIR=/home/hakan/sandbox/pi-hckarta2/n10/hckarta/target nice -n 19 ~/.cargo/bin/cargo build -j 4
./target/debug/hckarta --lat0 47.0 --lon0 -108.0 --b 64 --h 48 --ruta-m 2000 --t 500000 \
  --tidtabell tidtabell.tsv --fro 7
./target/debug/hckarta … --fro 181 --flodversion 2 --skriv /tmp/karta-v2
```

`Kalla::las` tar sökvägen till `n10/paleokarta/data` (rotationerna och Map16). `--fro` kräver `--tidtabell`
och skriver antalet rutor per vattentyp. `--flodversion` är 1 (standard) eller 2.
`--tidsmodell` är 1 (standard, `karta_full_version`) eller 2 (`karta_for_ar`).

## Prov

S1 (fem i `tests/s1.rs`): samma summa, ändrad ruta, Gilbert Creek 194 m ± 1 m, märkena Last/Simulering,
tomt område är fel.

S2 (sex i `tests/s2.rs`): tidtabellen läses, perioderna vid tre tider, simulering ändrar inget,
omkastad tabell annan karta, tom tabell är fel, summan ser varje lager (inkl. växt och period).

S3 (fem i `tests/s3.rs`): skriv och läs samma summa, ändrad byte ger fel, fel längd ger fel,
PNG är giltig, strandlinjen på x = 2,0 respektive tom för bara land.

S4 (sju i `tests/s4.rs`): samma/annat frö, fåror når östkanten, fåror går österut, Aldre fler fåror
än Ovre, hav rörs inte, märkena efter flod, filer med flodnät.

Claudes `tests/granser.rs`: perioderna vid tidtabellens gränser.

Alt 1343/1344 (`tests/fara1343.rs`): version 1 orörd (sha256), hcsim orörd, mittlinjen lika,
ytan/intervall, inga Flodfara i v2, sammanhang, samma/annat frö, 3D-format, märkning Simulering.
