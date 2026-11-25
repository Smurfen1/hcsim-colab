# Sparat läge (Alt 1302) — sammanfattning

Byggt 2026-11-21 av Cursor på grenen `hcsim-spara1302`. Underlag:
`n10/docs/2026-11-21-lasunderlag-spara1302.md`. Fortsättning efter granskning
(prov 5 med vägsamlare, prov 2–3 i egen process med `kill -9`, mätning av djurlistan).

## Vad som lades till

- `hcsim kor … --spara FIL [--spara-var-ar N]` — sparar hela `Sim` var N:e år och vid slut.
- `hcsim fortsatt --fran FIL --ar M` — laddar, bygger kartan, kräver samma kartsumma, kör M år.
- Serialisering: `#[derive(Serialize, Deserialize)]` på hela `Sim`-trädet (inte handskriven fältlista).
- Format: kuvert `HCSM` + versionsnummer + FNV-kontrollsumma + **binär bitbin-payload** med
  `f64::to_bits` (JSON tappade 1 ULP och bröt bit-exakt rundtur).
- Säker skrivning: `.tmp` → fsync → behåll `.forra` → rename.
- Utan nya val: `kor` går samma väg som före (`Sim::kor_ar`).
- `vagsamlare`: serde-`Serializer` som bara samlar kanoniska fältvägar (`djur[].hj.wp`, …).

## Prov 1–8

| Prov | Utfall | Anmärkning |
|------|--------|------------|
| 1 Dagens körning | **grön** | djur_summa = 4211610881721223074 |
| 2 Rundtur 20+20 | **grön** | `Command` → release-`hcsim`; samma summa; slutfiler byte-lika |
| 3 Avbrott `kill -9` | **grön** | efter ≥2 sparningar; `fortsatt` till 40; `.forra` laddbar |
| 4 Osparad `g` (PROV) | **grön** | annan summa när `g` nollställs före spara |
| 5 Varje fält | **grön** | 79 serialiserade vägar; täckningskrav; 195 mutationer; 148 levande, 47 döda |
| 6 Version/summa | **grön** | vägrar fel version och fel checksumma |
| 7 Diff stannar | **grön** | `git diff main -- n10/hcsim` bara tillåtna filer |
| 8 tal_tsv m.m. | **grön** | tal_tsv oförändrad |

## Döda fält i dag (prov 5)

```
rak.fodda_ar / rak.dod_svalt_ar / rak.dod_alder_ar
senaste_turordning          # tom i vanlig körning; push ändrar inte djur_summa på 2 år
v[0]                        # havsruta; nollställs i tillväxten
djur[i].hj.senaste_fel      # stickprov levande; påverkar inte djur_summa på 2 år
djur[i].hj.senaste_fel_handling
djur[i].forra_intag
djur[död].*                 # döda djur ingår inte i djur_summa (här index 51)
```

Undantag i täckningskravet (privata Klimat-fält): `klimat.q_medel`, `klimat.g_norm`.

## Mätning (N10-värld 64×48, efter 5 år)

| Mått | Värde |
|------|-------|
| Sparfilens storlek | **119 376 433 bytes** (~114 MiB) |
| Tid spara | **748 ms** |
| Tid ladda | **489 ms** |

## Djurlistan växer

N10-världen, frö 181. Döda tas inte bort med `retain`/`remove`; födslar kan återanvända
en död plats (`djur[p] = unge`), så `djur.len()` planar ut tidigt medan antalet levande varierar.
Ingen ändring av den hanteringen här — beslutas efteråt.

| År | `djur.len()` | Levande | Sparfil (bytes) | s/år (period) |
|----|-------------:|--------:|----------------:|--------------:|
| 5  | 26356 | 4992 | 119 376 433 | 6,08 |
| 10 | 26356 | 8364 | 119 375 173 | 2,17 |
| 20 | 26356 | 5176 | 119 375 533 | 2,47 |
| 40 | 26356 | 4894 | 119 376 073 | 2,30 |

(`cargo test --test spara1302`, 2026-11-21 kväll; s/år varierar med last från strykprovet V.)

## Fel med flit (röda prover)

1. **Fel förväntad djur_summa** i kontroll1 → röd (assertion).
2. **Nytt serde-fält `signal_tmp` i Hj-kopia** → täckningskravet faller med `signal_tmp` i meddelandet (`kontroll5_rod_nytt_falt_i_kopia`, grön som demonstration).
3. **`fortsatt` utan laddad `g`** → annan summa (`kontroll_rod_fortsatt_utan_g_visar_skillnad`); `assert_eq` mot 4211… faller (`--ignored`).
4. **`spara_utan_g_PROV` / vanlig spara i kontroll4** → `assert_ne!` faller om g sparas.
5. **Fel versionsnummer / en ändrad payload-byte** → kontroll6.
6. **Otillåten fil i diff** → kontroll7.
7. **Ändrat tal i tsv** → kontroll8.

## Avvikelser

- Payload är **binär bitbin**, inte JSON: serde_json rundtur är inte bit-exakt för f64.
- Prov 5 jämför mutationslistan mot **vägar ur serialiseringen** (vägsamlare), med stickprov
  för listor och uttryckliga undantag för privata klimatfält.
- Grenen **slås inte ihop med main** här (Fable: efter strykprovet V).

## Git

Se slutrapporten i chatten: `git rev-parse HEAD`, `git diff --stat main`.
