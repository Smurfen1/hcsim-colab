# paleokarta: en plats i dag till sitt läge för miljoner år sedan, och höjden där

Håkan 2026-11-19: "Hämta kartmodellens rotationer av kontinenterna så allt blir exakt". Rust (regel 11). Källor och licens: `data/KALLA.md`
och `n10/dinobiom/kallor-paleodem.md`.

## Vad den gör

1. Väljer plattan för platsen ur plattornas polygoner (`data/PALEOMAP_PlatePolygons.gpml`): den polygon som innehåller platsen i dag
   och som finns vid den valda tiden. Hell Creeks platser ligger på platta 101 (Nordamerika), som i PBDB.
2. Räknar plattans rotation mot jordens fasta ram ur rotationsfilen (`data/PALEOMAP_PlateModel.rot`) med GPlates regler: sfärisk
   interpolation mellan två rader för samma par av plattor, ingen interpolation över ett byte av fast platta, och kedjan av fasta
   plattor (för Nordamerika vid 66 miljoner år: 101 → 714 → 701 → 0).
3. Läser höjden i PaleoDEM-kartan (`data/Map16_065Ma.csv` för 66 miljoner år, `Map17_070Ma.csv` för 69) där platsen låg, och
   höjden längs bredgraden väster- och österut.

## Kör

```
cd n10/paleokarta
~/.cargo/bin/cargo build --release
./target/release/paleokarta --lat 47.637699 --lon -106.569901 --tid 66 --vaster 22 --oster 12
./target/release/paleokarta --lat 47.637699 --lon -106.569901 --tid 69 --dem data/Map17_070Ma.csv
```

Utfallen för Hell Creek står i `utfall/` (`gilbert-creek-66.txt`, `fem-platser.tsv`).

## Prov (5 gröna)

- Räknesättet mot fall som går att räkna för hand (90° runt nordpolen, polen står still, halva tiden ger halva vinkeln, kedjans
  ordning, byte av fast platta, ett glapp utan gemensam tid ger fel), polygoner över datumlinjen, en polygon i Asien som inte får
  innehålla Montana.
- **Mot en oberoende räknare:** GPlates webbtjänst (gws.gplates.org, modellen PALEOMAP), tolv platser på olika plattor och sju tider
  (0, 30, 66, 69, 100, 150 och 250 miljoner år), svaren sparade i `tests/gplates/` (hämtade 2026-11-19 17:15). Gränsen 0,001° sattes innan
  provet kördes. Utfall: 84 av 84 lika (en av dem är Tokyo vid 250 miljoner år, där både GPlates och koden saknar läge,
  eftersom platta 610 finns först från 245 miljoner år).
- Hell Creeks fem platser ligger på platta 101.

## Felprov

6 fel sattes in ett i taget i en kopia; alla 6 gav rött prov: vinkeln åt fel håll; kedjan i omvänd ordning; ingen interpolation mellan raderna; polygonens hörn läses som längdgrad, bredgrad; polygonens tid prövas inte; radpar över en övergång mellan fasta plattor. Programmet:
`src/bin/paleokarta-felprov.rs` (`paleokarta-felprov <kopia av n10/paleokarta> <mål för cargo>`; rör aldrig arkivet). Det första
försöket visade att provet för byte av fast platta inte såg felet; provet fick då fallet med ett glapp (2026-11-19).

## Gränser

- Strålprovet för polygonerna räknar inte rätt för en polygon runt en pol.
- Kartan har en punkt per hel grad; höjden mellan punkterna är bilinjär, och avstånden är aldrig noggrannare än en grad.
- Första versionen av strålprovet flyttade hörnen var för sig mot punktens längdgrad, så en polygon i Asien (platta 402) såg ut att
  innehålla Montana. Rättat; provet `polygon_och_datumlinjen` har fallet.
