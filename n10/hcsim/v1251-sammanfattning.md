# Strykprovet på värdets signalväg (Alt 1251, med 1252 och 1253)

Gränser ur `n10/docs/2026-11-21-lasunderlag-v1251.md` (skrivna före körningen).

Total körtid: 32936.8 s

Logg: 824007343 byte, sha256=fnv64:157c0a7c878480e6 (summan är FNV-64, inte sha256; se Claudes granskning sist)

`v0` är lika för alla handlingar och tas bort genom centreringen per rad.

| Arm | Spegel | Klocka | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |
|-----|--------|--------|-------|--------|-------------------|-------|-----|-----|
| a_to | nej | flytt_i_tur | trangsel_orakel | slump | 18/20 | 0.7519 | 0.1939 | 1.0000 |
| a_to | ja | flytt_i_tur | slump | trangsel_orakel | 1/20 | 0.2474 | 0.0000 | 0.7826 |
| b_to | nej | bete_i_tur | trangsel_orakel | slump | 0/20 | 0.0903 | 0.0421 | 0.1325 |
| b_to | ja | bete_i_tur | slump | trangsel_orakel | 20/20 | 0.8854 | 0.8414 | 0.9504 |
| a_tab | nej | flytt_i_tur | tabell | slump | 0/20 | 0.0329 | 0.0183 | 0.0525 |
| a_tab | ja | flytt_i_tur | slump | tabell | 20/20 | 0.9648 | 0.9429 | 0.9824 |
| b_tab | nej | bete_i_tur | tabell | slump | 0/20 | 0.0347 | 0.0109 | 0.0592 |
| b_tab | ja | bete_i_tur | slump | tabell | 20/20 | 0.9681 | 0.9470 | 0.9865 |
| u_to | nej | turordning | trangsel_orakel | slump | 20/20 | 0.8501 | 0.6551 | 1.0000 |
| u_to | ja | turordning | slump | trangsel_orakel | 1/20 | 0.2252 | 0.0000 | 0.7801 |
| s_to | nej | samtidig | trangsel_orakel | slump | 0/20 | 0.0666 | 0.0000 | 0.2183 |
| s_to | ja | samtidig | slump | trangsel_orakel | 20/20 | 0.9103 | 0.7938 | 1.0000 |
| v_lar | nej | samtidig | lar | lar | 14/20 | 0.5636 | 0.3221 | 0.8919 |

## Alt 1251 mått per frö

| Frö | Kvot | Kvot lage | z rör | Ingrepp mark | Ingrepp slut | W | k_pos | n_djur |
|-----|------|-----------|-------|--------------|--------------|---|-------|--------|
| 201 | 0.2637043660784948 | 0.2850939386423481 | 0.2976112007556964 | 0.09335280852808528 | 0.04123392680514342 | 0.20238095238095238 | 0.48805071682747103 | 8 |
| 202 | 0.30659242214720417 | 0.306592448947005 | 0.25376211664931203 | 0.09420140321914981 | 0.04096149129261712 | 0.06139315230224321 | 0.5776275356653413 | 11 |
| 203 | 0.2071795848489881 | 0.2603143427152703 | 0.27183065543430296 | 0.09295935296557445 | 0.03877837442030754 | 0.08759553203997648 | 0.527338490648054 | 18 |
| 204 | 0.2635780717831395 | 0.22465789426844718 | 0.2625136932232757 | 0.09396853146853147 | 0.048146582950978026 | 0.048177083333333336 | 0.5577306392671755 | 16 |
| 205 | 0.26516249839259864 | 0.30445221728436567 | 0.27961925222260847 | 0.09267979452054795 | 0.03769306202500613 | 0.17564672927994726 | 0.44192314747344863 | 17 |
| 206 | 0.25398749899082634 | 0.24217816803091752 | 0.2533198976553939 | 0.0923188561599656 | 0.034392500608716826 | 0.07635953789799943 | 0.40906062882791033 | 13 |
| 207 | 0.29744144290195035 | 0.2136455396575876 | 0.2597368300240212 | 0.09444913465597299 | 0.03758169934640523 | 0.048012593467138924 | 0.6542932635291046 | 11 |
| 208 | 0.1737940263164491 | 0.139004311495065 | 0.21776619445072948 | 0.09144792548687553 | 0.03950256035113387 | 0.031961991145664614 | 0.4202313538527438 | 21 |
| 209 | 0.41335404408753473 | 0.40040530624611476 | 0.2915472685311759 | 0.10112497543721753 | 0.04379096868171886 | 0.12863756613756613 | 0.5232674726185714 | 12 |
| 210 | 0.2642519917775546 | 0.25165791926128284 | 0.289324610257571 | 0.09477277465082343 | 0.03868466047874939 | 0.09428571428571429 | 0.6070108283504023 | 10 |
| 211 | 0.36544673908817527 | 0.3003398553845171 | 0.29271489956105323 | 0.09054435046627629 | 0.036940116845180136 | 0.027901785714285716 | 0.5966710578357629 | 16 |
| 212 | 0.27969935529968903 | 0.36183224273941345 | 0.30722074792505355 | 0.08990045624222315 | 0.04330637373249638 | 0.08143138912369681 | 0.48515747249003477 | 13 |
| 213 | 0.3390495463370486 | 0.35758497706558984 | 0.30609889748011604 | 0.0926706827309237 | 0.0391840953307393 | 0.04057480980557904 | 0.565742176197215 | 13 |
| 214 | 0.2455726211719271 | 0.24204079681783236 | 0.29787217973148933 | 0.08953168044077135 | 0.043512658227848104 | 0.058658757620695334 | 0.52574537691255 | 17 |
| 215 | 0.25317300068556414 | 0.1973320794204371 | 0.24157710299926502 | 0.09343105863518111 | 0.039341179325406456 | 0.056851311953352766 | 0.5032773445102486 | 14 |
| 216 | 0.2538016323950626 | 0.3352006116670281 | 0.26741311608452223 | 0.09714200358494324 | 0.043095064429856555 | 0.07738095238095238 | 0.5194065306777285 | 8 |
| 217 | 0.28462511845608585 | 0.2683874196370097 | 0.30867497468507754 | 0.09651767151767152 | 0.04445670836399313 | 0.08381924198250729 | 0.4940197792749024 | 14 |
| 218 | 0.32894616645909047 | 0.31775609986643133 | 0.32028529491063046 | 0.09232506829165792 | 0.039207980652962514 | 0.07333921222810112 | 0.48795094708788217 | 18 |
| 219 | 0.1880308700187522 | 0.17767745223057688 | 0.23667474306126984 | 0.09307415036045315 | 0.040744344441741666 | 0.05952380952380952 | 0.44763430678019367 | 10 |
| 220 | 0.262053592808842 | 0.20034720503948367 | 0.24528086229090035 | 0.09600740893187899 | 0.03893034825870647 | 0.08238095238095237 | 0.5441376949740454 | 10 |

## Mått 5: blandning på rangloggen

| Frö | Oblandad | Inom djur | Mellan djur |
|-----|----------|-----------|-------------|
| 161 | -0.0795147676416433 | -0.07478447120905113 | -0.018215862561091666 |
| 162 | -0.08075816934875173 | -0.08244418757761755 | -0.010468549262298798 |
| 163 | -0.08501218953906066 | -0.08607847366653344 | -0.013208048504629786 |
| 164 | -0.08776959646608559 | -0.087035621644544 | -0.020626808988841318 |
| 165 | -0.0856072694176813 | -0.08392273780643546 | -0.013491580627397493 |
| 166 | -0.0819343399749479 | -0.08449902826721273 | -0.014034086826690795 |
| 167 | -0.06908134240510998 | -0.07234456691247772 | -0.020972592702635972 |
| 168 | -0.0824708992068129 | -0.08029319912090928 | -0.008361317130130972 |
| 169 | -0.07907682151010947 | -0.07572096159827936 | -0.005202243434018533 |
| 170 | -0.07637524923951142 | -0.07979176072202734 | -0.007683761817939644 |
| 171 | -0.07589850704863679 | -0.07671852373113575 | -0.01293975390206558 |
| 172 | -0.07915274876164155 | -0.07877068830986324 | -0.014013393174006412 |
| 173 | -0.07837612313450558 | -0.08197258153021564 | -0.015803013384349442 |
| 174 | -0.0913915908780515 | -0.09104590040210836 | -0.01975807841243185 |
| 175 | -0.08027118016836979 | -0.08210909383830318 | -0.012444054253805367 |
| 176 | -0.06840671287126499 | -0.06990921357292611 | -0.015208531344493065 |
| 177 | -0.08236434597805391 | -0.07988245365285279 | -0.01619312701151403 |
| 178 | -0.09073167056424773 | -0.0876344286152869 | -0.01768641266932685 |
| 179 | -0.0774836791928311 | -0.07888830941339904 | -0.014895796143030653 |
| 180 | -0.07362182705390413 | -0.0724332537522366 | -0.011330348338290075 |

## Bokföring (Alt 1253, u_to/a_to/b_to utan spegel)

Antal blockrader: 2640.

Frön där trängseloraklet åt mer än slumpen i u_to (181, 189, 192, 195, 200):

- frö 181 (u_to): 44 blockrader
  - slump block 1: födda=390, död_svalt=1890, död_alder=164, antal_slut=729, medelintag=0.916620671492169
  - slump block 2: födda=16, död_svalt=356, död_alder=128, antal_slut=261, medelintag=1.0341080681846835
  - slump block 3: födda=27, död_svalt=95, död_alder=144, antal_slut=49, medelintag=1.024296204911895
  - slump block 4: födda=7, död_svalt=13, död_alder=20, antal_slut=23, medelintag=1.2438960391671776
  - slump block 5: födda=0, död_svalt=7, död_alder=5, antal_slut=11, medelintag=1.2486515826289857
  - slump block 6: födda=0, död_svalt=0, död_alder=9, antal_slut=2, medelintag=2.189892560846659
  - slump block 7: födda=0, död_svalt=0, död_alder=2, antal_slut=0, medelintag=1.6844598345703468
  - slump block 8: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 9: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 10: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 11: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 12: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 13: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 14: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 15: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 16: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 17: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 18: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 19: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 20: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 21: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 22: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - trangsel_orakel block 1: födda=1430, död_svalt=364, död_alder=183, antal_slut=3275, medelintag=1.102194804373109
  - trangsel_orakel block 2: födda=916, död_svalt=6, död_alder=371, antal_slut=3814, medelintag=1.2257549671662522
  - trangsel_orakel block 3: födda=1139, död_svalt=10, död_alder=1630, antal_slut=3313, medelintag=1.1505562262372684
  - trangsel_orakel block 4: födda=2651, död_svalt=10, död_alder=1259, antal_slut=4695, medelintag=1.2017794240805753
  - trangsel_orakel block 5: födda=104, död_svalt=705, död_alder=810, antal_slut=3284, medelintag=0.9614294765174666
  - trangsel_orakel block 6: födda=2011, död_svalt=26, död_alder=1032, antal_slut=4237, medelintag=1.2624266081002236
  - trangsel_orakel block 7: födda=2599, död_svalt=35, död_alder=2134, antal_slut=4667, medelintag=1.2468496799025366
  - trangsel_orakel block 8: födda=96, död_svalt=698, död_alder=71, antal_slut=3994, medelintag=0.9124078042828104
  - trangsel_orakel block 9: födda=2212, död_svalt=422, död_alder=1620, antal_slut=4164, medelintag=1.351629153892872
  - trangsel_orakel block 10: födda=1366, död_svalt=248, död_alder=1890, antal_slut=3392, medelintag=1.0530801975223296
  - trangsel_orakel block 11: födda=1141, död_svalt=116, död_alder=23, antal_slut=4394, medelintag=1.1496077572778494
  - trangsel_orakel block 12: födda=387, död_svalt=299, död_alder=1904, antal_slut=2578, medelintag=1.0630885818659812
  - trangsel_orakel block 13: födda=3675, död_svalt=233, död_alder=1304, antal_slut=4716, medelintag=1.220365551377011
  - trangsel_orakel block 14: födda=1078, död_svalt=1233, död_alder=715, antal_slut=3846, medelintag=1.1185307891302279
  - trangsel_orakel block 15: födda=688, död_svalt=465, död_alder=227, antal_slut=3842, medelintag=1.068976733519678
  - trangsel_orakel block 16: födda=3524, död_svalt=71, död_alder=2391, antal_slut=4904, medelintag=1.3372339821395731
  - trangsel_orakel block 17: födda=260, död_svalt=320, död_alder=682, antal_slut=4162, medelintag=0.9591141402189852
  - trangsel_orakel block 18: födda=1382, död_svalt=674, död_alder=600, antal_slut=4270, medelintag=1.1752937927354952
  - trangsel_orakel block 19: födda=1739, död_svalt=188, död_alder=2702, antal_slut=3119, medelintag=1.200503712211692
  - trangsel_orakel block 20: födda=1974, död_svalt=950, död_alder=159, antal_slut=3984, medelintag=1.053088059814382
  - trangsel_orakel block 21: födda=1116, död_svalt=763, död_alder=948, antal_slut=3389, medelintag=1.1514492356447976
  - trangsel_orakel block 22: födda=1496, död_svalt=393, död_alder=1199, antal_slut=3293, medelintag=1.1360151938097893
- frö 189 (u_to): 44 blockrader
  - slump block 1: födda=422, död_svalt=1978, död_alder=130, antal_slut=734, medelintag=0.912139686209283
  - slump block 2: födda=10, död_svalt=353, död_alder=142, antal_slut=249, medelintag=1.0376797032106782
  - slump block 3: födda=35, död_svalt=97, död_alder=141, antal_slut=46, medelintag=1.0333658660498148
  - slump block 4: födda=29, död_svalt=17, död_alder=17, antal_slut=41, medelintag=1.1840291562094283
  - slump block 5: födda=7, död_svalt=31, död_alder=1, antal_slut=16, medelintag=1.075676102870816
  - slump block 6: födda=1, död_svalt=3, död_alder=5, antal_slut=9, medelintag=1.427266209106996
  - slump block 7: födda=1, död_svalt=1, död_alder=6, antal_slut=3, medelintag=1.553554350293604
  - slump block 8: födda=0, död_svalt=0, död_alder=1, antal_slut=2, medelintag=1.2150070237617503
  - slump block 9: födda=0, död_svalt=0, död_alder=1, antal_slut=1, medelintag=1.1509645925018976
  - slump block 10: födda=0, död_svalt=0, död_alder=1, antal_slut=0, medelintag=0.9017566142440039
  - slump block 11: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 12: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 13: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 14: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 15: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 16: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 17: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 18: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 19: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 20: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 21: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 22: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - trangsel_orakel block 1: födda=1402, död_svalt=397, död_alder=162, antal_slut=3262, medelintag=1.0964635032373715
  - trangsel_orakel block 2: födda=858, död_svalt=66, död_alder=353, antal_slut=3701, medelintag=1.223546693218529
  - trangsel_orakel block 3: födda=1259, död_svalt=20, död_alder=1657, antal_slut=3283, medelintag=1.1516429052927641
  - trangsel_orakel block 4: födda=2698, död_svalt=20, död_alder=1181, antal_slut=4780, medelintag=1.1896058497202184
  - trangsel_orakel block 5: födda=248, död_svalt=825, död_alder=736, antal_slut=3467, medelintag=0.9789204802327381
  - trangsel_orakel block 6: födda=2067, död_svalt=173, död_alder=1098, antal_slut=4263, medelintag=1.2468445341686578
  - trangsel_orakel block 7: födda=2544, död_svalt=22, död_alder=2078, antal_slut=4707, medelintag=1.2573897871699238
  - trangsel_orakel block 8: födda=119, död_svalt=520, död_alder=163, antal_slut=4143, medelintag=0.9150907251660683
  - trangsel_orakel block 9: födda=1929, död_svalt=458, död_alder=1713, antal_slut=3901, medelintag=1.2886950708107856
  - trangsel_orakel block 10: födda=1903, död_svalt=170, död_alder=1911, antal_slut=3723, medelintag=1.083033504934353
  - trangsel_orakel block 11: födda=570, död_svalt=230, död_alder=36, antal_slut=4027, medelintag=1.1082941068194454
  - trangsel_orakel block 12: födda=969, död_svalt=237, död_alder=1669, antal_slut=3090, medelintag=1.062754737011882
  - trangsel_orakel block 13: födda=3936, död_svalt=259, död_alder=1754, antal_slut=5013, medelintag=1.2646025998326604
  - trangsel_orakel block 14: födda=941, död_svalt=1403, död_alder=320, antal_slut=4231, medelintag=1.0288931991958636
  - trangsel_orakel block 15: födda=842, död_svalt=379, död_alder=611, antal_slut=4083, medelintag=1.1450375768415173
  - trangsel_orakel block 16: födda=4190, död_svalt=195, död_alder=2569, antal_slut=5509, medelintag=1.3369460843513121
  - trangsel_orakel block 17: födda=392, död_svalt=2811, död_alder=560, antal_slut=2530, medelintag=0.9618132049368494
  - trangsel_orakel block 18: födda=510, död_svalt=230, död_alder=424, antal_slut=2386, medelintag=1.8052228579876777
  - trangsel_orakel block 19: födda=650, död_svalt=101, död_alder=1618, antal_slut=1317, medelintag=2.5519282133836008
  - trangsel_orakel block 20: födda=900, död_svalt=662, död_alder=192, antal_slut=1363, medelintag=1.0794263814702683
  - trangsel_orakel block 21: födda=1088, död_svalt=272, död_alder=345, antal_slut=1834, medelintag=1.348723652149861
  - trangsel_orakel block 22: födda=366, död_svalt=471, död_alder=337, antal_slut=1392, medelintag=1.0451591704128582
- frö 192 (u_to): 44 blockrader
  - slump block 1: födda=388, död_svalt=1917, död_alder=136, antal_slut=745, medelintag=0.9032747029910136
  - slump block 2: födda=5, död_svalt=334, död_alder=144, antal_slut=272, medelintag=1.0319831577927663
  - slump block 3: födda=35, död_svalt=84, död_alder=175, antal_slut=48, medelintag=1.0522118169739259
  - slump block 4: födda=33, död_svalt=13, död_alder=23, antal_slut=45, medelintag=1.3026766049749108
  - slump block 5: födda=0, död_svalt=21, död_alder=1, antal_slut=23, medelintag=1.0905967038884992
  - slump block 6: födda=9, död_svalt=4, död_alder=10, antal_slut=18, medelintag=1.3904732896076832
  - slump block 7: födda=0, död_svalt=5, död_alder=9, antal_slut=4, medelintag=1.1448298374953019
  - slump block 8: födda=0, död_svalt=0, död_alder=0, antal_slut=4, medelintag=1.6310657801206738
  - slump block 9: födda=0, död_svalt=0, död_alder=4, antal_slut=0, medelintag=2.1058955688928616
  - slump block 10: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 11: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 12: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 13: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 14: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 15: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 16: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 17: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 18: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 19: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 20: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 21: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 22: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - trangsel_orakel block 1: födda=1394, död_svalt=304, död_alder=179, antal_slut=3320, medelintag=1.0814579054701419
  - trangsel_orakel block 2: födda=771, död_svalt=7, död_alder=372, antal_slut=3712, medelintag=1.2059641799792726
  - trangsel_orakel block 3: födda=1310, död_svalt=17, död_alder=1680, antal_slut=3325, medelintag=1.1497306677532988
  - trangsel_orakel block 4: födda=2749, död_svalt=8, död_alder=1255, antal_slut=4811, medelintag=1.1875141283341104
  - trangsel_orakel block 5: födda=93, död_svalt=906, död_alder=707, antal_slut=3291, medelintag=0.9561754004991759
  - trangsel_orakel block 6: födda=2216, död_svalt=72, död_alder=1131, antal_slut=4304, medelintag=1.264840708667373
  - trangsel_orakel block 7: födda=2377, död_svalt=34, död_alder=2047, antal_slut=4600, medelintag=1.2350850386215972
  - trangsel_orakel block 8: födda=201, död_svalt=583, död_alder=46, antal_slut=4172, medelintag=0.9119220613236104
  - trangsel_orakel block 9: födda=2075, död_svalt=475, död_alder=1865, antal_slut=3907, medelintag=1.2985375034571351
  - trangsel_orakel block 10: födda=1647, död_svalt=172, död_alder=1805, antal_slut=3577, medelintag=1.0800170807484064
  - trangsel_orakel block 11: födda=1183, död_svalt=83, död_alder=16, antal_slut=4661, medelintag=1.1060643363056024
  - trangsel_orakel block 12: födda=267, död_svalt=607, död_alder=1777, antal_slut=2544, medelintag=0.970433896278357
  - trangsel_orakel block 13: födda=5307, död_svalt=189, död_alder=1488, antal_slut=6174, medelintag=1.2597095868180928
  - trangsel_orakel block 14: födda=984, död_svalt=3136, död_alder=540, antal_slut=3482, medelintag=1.2697946705119996
  - trangsel_orakel block 15: födda=1144, död_svalt=190, död_alder=146, antal_slut=4290, medelintag=1.1829471345114382
  - trangsel_orakel block 16: födda=2346, död_svalt=49, död_alder=2269, antal_slut=4318, medelintag=1.2488730446786198
  - trangsel_orakel block 17: födda=933, död_svalt=305, död_alder=815, antal_slut=4131, medelintag=0.9674684466709871
  - trangsel_orakel block 18: födda=1213, död_svalt=171, död_alder=1139, antal_slut=4034, medelintag=1.1809758868190936
  - trangsel_orakel block 19: födda=1640, död_svalt=2369, död_alder=1609, antal_slut=1696, medelintag=1.0034962305434965
  - trangsel_orakel block 20: födda=5113, död_svalt=523, död_alder=482, antal_slut=5804, medelintag=1.35495429332613
  - trangsel_orakel block 21: födda=567, död_svalt=2952, död_alder=594, antal_slut=2825, medelintag=1.101135503983455
  - trangsel_orakel block 22: födda=377, död_svalt=152, död_alder=276, antal_slut=2774, medelintag=1.7402548644548628
- frö 195 (u_to): 44 blockrader
  - slump block 1: födda=380, död_svalt=1959, död_alder=135, antal_slut=641, medelintag=0.8978794689404083
  - slump block 2: födda=30, död_svalt=288, död_alder=137, antal_slut=246, medelintag=1.07318767490206
  - slump block 3: födda=9, död_svalt=109, död_alder=114, antal_slut=32, medelintag=0.9785583187155263
  - slump block 4: födda=11, död_svalt=12, död_alder=14, antal_slut=17, medelintag=1.1481539264311424
  - slump block 5: födda=0, död_svalt=12, död_alder=3, antal_slut=2, medelintag=0.8855629386638254
  - slump block 6: födda=0, död_svalt=0, död_alder=1, antal_slut=1, medelintag=1.2945170522521525
  - slump block 7: födda=0, död_svalt=0, död_alder=1, antal_slut=0, medelintag=0.8076154398122357
  - slump block 8: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 9: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 10: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 11: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 12: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 13: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 14: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 15: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 16: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 17: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 18: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 19: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 20: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 21: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 22: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - trangsel_orakel block 1: födda=1677, död_svalt=660, död_alder=178, antal_slut=3193, medelintag=1.0817113212682443
  - trangsel_orakel block 2: födda=1181, död_svalt=8, död_alder=342, antal_slut=4024, medelintag=1.257497645154753
  - trangsel_orakel block 3: födda=1033, död_svalt=28, död_alder=1527, antal_slut=3502, medelintag=1.1311182807920128
  - trangsel_orakel block 4: födda=2690, död_svalt=23, död_alder=1304, antal_slut=4865, medelintag=1.1950519682045913
  - trangsel_orakel block 5: födda=152, död_svalt=1021, död_alder=999, antal_slut=2997, medelintag=0.970506876919914
  - trangsel_orakel block 6: födda=2219, död_svalt=93, död_alder=865, antal_slut=4258, medelintag=1.2408740420725601
  - trangsel_orakel block 7: födda=2563, död_svalt=74, död_alder=1937, antal_slut=4810, medelintag=1.2594347926659109
  - trangsel_orakel block 8: födda=157, död_svalt=1184, död_alder=86, antal_slut=3697, medelintag=0.9003434285003218
  - trangsel_orakel block 9: födda=2955, död_svalt=214, död_alder=1762, antal_slut=4676, medelintag=1.4277578180579171
  - trangsel_orakel block 10: födda=663, död_svalt=266, död_alder=1652, antal_slut=3421, medelintag=1.0124375464955453
  - trangsel_orakel block 11: födda=1219, död_svalt=152, död_alder=24, antal_slut=4464, medelintag=1.1316509595292394
  - trangsel_orakel block 12: födda=786, död_svalt=271, död_alder=2629, antal_slut=2350, medelintag=1.0847423241034173
  - trangsel_orakel block 13: födda=4053, död_svalt=2540, död_alder=578, antal_slut=3285, medelintag=1.132809288442617
  - trangsel_orakel block 14: födda=2063, död_svalt=10, död_alder=714, antal_slut=4624, medelintag=1.3532360677615802
  - trangsel_orakel block 15: födda=197, död_svalt=377, död_alder=404, antal_slut=4040, medelintag=0.9567297826817156
  - trangsel_orakel block 16: födda=3324, död_svalt=151, död_alder=2006, antal_slut=5207, medelintag=1.33701793858555
  - trangsel_orakel block 17: födda=456, död_svalt=2345, död_alder=1476, antal_slut=1842, medelintag=0.9273198599720379
  - trangsel_orakel block 18: födda=5491, död_svalt=2819, död_alder=90, antal_slut=4424, medelintag=1.3167400898709152
  - trangsel_orakel block 19: födda=504, död_svalt=924, död_alder=1197, antal_slut=2807, medelintag=1.1452525282360462
  - trangsel_orakel block 20: födda=1427, död_svalt=716, död_alder=190, antal_slut=3328, medelintag=1.3980417606097166
  - trangsel_orakel block 21: födda=3688, död_svalt=406, död_alder=1926, antal_slut=4684, medelintag=1.5859576388690606
  - trangsel_orakel block 22: födda=1159, död_svalt=1880, död_alder=225, antal_slut=3738, medelintag=0.9416901009192954
- frö 200 (u_to): 44 blockrader
  - slump block 1: födda=363, död_svalt=1800, död_alder=133, antal_slut=750, medelintag=0.9029336932041089
  - slump block 2: födda=12, död_svalt=383, död_alder=130, antal_slut=249, medelintag=1.0153163438401895
  - slump block 3: födda=34, död_svalt=80, död_alder=162, antal_slut=41, medelintag=1.033650773706281
  - slump block 4: födda=19, död_svalt=15, död_alder=14, antal_slut=31, medelintag=1.1895484539787868
  - slump block 5: födda=1, död_svalt=17, död_alder=2, antal_slut=13, medelintag=1.002979136223605
  - slump block 6: födda=0, död_svalt=3, död_alder=6, antal_slut=4, medelintag=1.3206971331496178
  - slump block 7: födda=0, död_svalt=0, död_alder=3, antal_slut=1, medelintag=1.1364823605125756
  - slump block 8: födda=0, död_svalt=1, död_alder=0, antal_slut=0, medelintag=0.7661569148241164
  - slump block 9: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 10: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 11: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 12: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 13: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 14: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 15: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 16: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 17: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 18: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 19: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 20: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 21: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - slump block 22: födda=0, död_svalt=0, död_alder=0, antal_slut=0, medelintag=null
  - trangsel_orakel block 1: födda=1567, död_svalt=327, död_alder=185, antal_slut=3374, medelintag=1.0791978508556617
  - trangsel_orakel block 2: födda=827, död_svalt=83, död_alder=351, antal_slut=3767, medelintag=1.2027044972127368
  - trangsel_orakel block 3: födda=1296, död_svalt=0, död_alder=1586, antal_slut=3477, medelintag=1.137437531383822
  - trangsel_orakel block 4: födda=2797, död_svalt=18, död_alder=1354, antal_slut=4902, medelintag=1.1773156858821012
  - trangsel_orakel block 5: födda=78, död_svalt=905, död_alder=696, antal_slut=3379, medelintag=0.9487971679220975
  - trangsel_orakel block 6: födda=2257, död_svalt=58, död_alder=1160, antal_slut=4418, medelintag=1.264618963543949
  - trangsel_orakel block 7: födda=2374, död_svalt=34, död_alder=2131, antal_slut=4627, medelintag=1.234886141390124
  - trangsel_orakel block 8: födda=123, död_svalt=182, död_alder=49, antal_slut=4519, medelintag=0.924345114177617
  - trangsel_orakel block 9: födda=1958, död_svalt=721, död_alder=1943, antal_slut=3813, medelintag=1.2620801418082768
  - trangsel_orakel block 10: födda=2074, död_svalt=1201, död_alder=1836, antal_slut=2850, medelintag=1.043649218322621
  - trangsel_orakel block 11: födda=1977, död_svalt=51, död_alder=0, antal_slut=4776, medelintag=1.1793704108067604
  - trangsel_orakel block 12: födda=208, död_svalt=1221, död_alder=1237, antal_slut=2526, medelintag=0.9496204820041831
  - trangsel_orakel block 13: födda=3551, död_svalt=162, död_alder=1170, antal_slut=4745, medelintag=1.3357509121452151
  - trangsel_orakel block 14: födda=617, död_svalt=29, död_alder=1116, antal_slut=4217, medelintag=0.9869386441328954
  - trangsel_orakel block 15: födda=3, död_svalt=163, död_alder=118, antal_slut=3939, medelintag=1.0422905656978394
  - trangsel_orakel block 16: födda=1034, död_svalt=51, död_alder=3413, antal_slut=1509, medelintag=1.2491409955507482
  - trangsel_orakel block 17: födda=724, död_svalt=262, död_alder=452, antal_slut=1519, medelintag=1.0621418826682782
  - trangsel_orakel block 18: födda=193, död_svalt=234, död_alder=2, antal_slut=1476, medelintag=1.0854888188190626
  - trangsel_orakel block 19: födda=458, död_svalt=64, död_alder=774, antal_slut=1096, medelintag=1.2251721939972475
  - trangsel_orakel block 20: födda=1579, död_svalt=123, död_alder=533, antal_slut=2019, medelintag=1.1700595558118818
  - trangsel_orakel block 21: födda=473, död_svalt=876, död_alder=73, antal_slut=1543, medelintag=1.205252358394327
  - trangsel_orakel block 22: födda=249, död_svalt=142, död_alder=276, antal_slut=1374, medelintag=1.0631842582570559

## Avgöranden

- **Spegel a_to:** medel av (andel + spegelandel) = 0.9993 (krav 1 ± 0.116) — inom gränsen.
- **Spegel b_to:** medel av (andel + spegelandel) = 0.9757 (krav 1 ± 0.116) — inom gränsen.
- **Spegel a_tab:** medel av (andel + spegelandel) = 0.9977 (krav 1 ± 0.116) — inom gränsen.
- **Spegel b_tab:** medel av (andel + spegelandel) = 1.0028 (krav 1 ± 0.116) — inom gränsen.
- **Spegel u_to:** medel av (andel + spegelandel) = 1.0753 (krav 1 ± 0.116) — inom gränsen.
- **Spegel s_to:** medel av (andel + spegelandel) = 0.9769 (krav 1 ± 0.116) — inom gränsen.
- **Instrumentet:** positiv kvot ≥0,5 i 12/20 frön. Instrumentet underkänt; inget utfall för lar tolkas.
- **Alt 1251:** tolkas inte (instrument underkänt).
- **Flockens drift:** tolkas inte.
- **Fast per djur:** blandning mellan djur ger rangkorrelation över −0,02 medan blandning inom djur ger under −0,05 i 18/20: felet är djurets fasta ordning.
- **Alt 1252:** a_to 18/20 (spegel ok), b_to 0/20 (spegel ok). a_to minst 15 och b_to högst 5: uppgiften om de andras nya platser bär segern.
- **Upprepningen:** u_to 20/20 och s_to 20/20 exakt samma andel som Alt 1246.
- **Bokföringen:** bokföringskontrollen godkänd för alla körda armar (antal vid märkning + födslar − döda = antal vid slut).
- **a_tab (redovisning):** 0/20 vunna, medel andel 0.0329.
- **b_tab (redovisning):** 0/20 vunna, medel andel 0.0347.
- **tolkas inte.**

## Claudes granskning (2026-11-22 00:01)

- **Loggen** flyttad ur arkivet till `/home/hakan/sandbox/pi/work/hcsim/v1251-logg.tsv`: 824007343 byte, sha256
  a8f7f75087912ec4e8788605bc1f6402fa520094623778e29f3592fd4e15cebf. Raden "sha256=fnv64:…" ovan är en FNV-64-summa med fel etikett.
- **Upprepningen** prövad av Claude: u_to och s_to (80 körningar, arm, spegel, frö, andel) är lika med `tur1246-korningar.jsonl` på main; jämförelsen
  blir röd med ett ändrat tal.
- **Bokföringen** prövad av Claude: kedjan antal före + födda − döda = antal efter håller i 2 520 blockrader (0 fel; röd med en födsel borttagen);
  summan av blockens startantal är lika med antalet vid märkningen i alla 60 körningar. Tabellen har bara hela block om 1 000 steg, så det sista
  ofullständiga blocket saknas i `v1251-bokforing.tsv`; programmets kontroll (`Bokforing::kontroll`) använder totalerna och omfattar hela körningen.
- **Instrumentet, två brister:** (1) den positiva avläsarens träff i ingreppet räknas inte: `ingrepp_traff_positiv` ger alltid 1,0, fast underlaget
  kräver att samma program räknar den; (2) handling 0 (stå kvar) läser den egna rutans växtandel `s[0]`, som djuret själv betar ned. Claudes tolkning,
  inte mätt: det ger den positiva avläsaren en fast skillnad mellan handlingarna och kan förklara att `k_pos` bara är 0,41–0,65. Gränsen 0,5 var skriven
  före körningen och står kvar; utfallet för Alt 1251 tolkas inte.
- **Iakttagelse, inte tolkad** (instrumentet underkänt): ingreppets träff för `lar` är 0,089–0,101 vid märkningen och 0,034–0,048 vid slutet, under
  slumpens 1/8 = 0,125.
- cargo test --release på 72b996a: 54 av 54 i `prover.rs` och 1 av 1 i `diagnos_tabell` gröna.
