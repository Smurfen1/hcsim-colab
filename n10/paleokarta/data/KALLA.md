# Källor och licens för data/ (paleokartan)

Alla filer här är oförändrade kopior ur Zenodo-posten 5460860 (DOI 10.5281/zenodo.5460860), licens CC BY 4.0 (fältet `license` i posten:
`cc-by-4.0`; EarthBytes `License.txt` i samma samling säger detsamma). Upphovet anges så här:
"PALEOMAP-modellen och höjddata: C. R. Scotese och N. M. Wright (2018), PALEOMAP Paleodigital Elevation Models (PaleoDEMS) for the
Phanerozoic, Zenodo, doi:10.5281/zenodo.5460860, CC BY 4.0".

| Fil | Ur zip-filen i posten | sha256 |
|---|---|---|
| `PALEOMAP_PlateModel.rot` | `Scotese_PaleoAtlas_v3.zip` (58 089 990 byte, md5:9a8d16ab2d7f070ae3e89da7835ce4d4), mappen `PALEOMAP Global Plate Model/` | `d56846bb3c6260805d088171066bc18e9d54e831892b660fd31315260e288022` |
| `PALEOMAP_PlatePolygons.gpml` | samma | `4925c064a415cfa6e7fd3f5e0682585baa9908f01bee9e053ca5dea0b0dc57ca` |
| `Map16_065Ma.csv` | `PaleoDEMS_long_lat_elev_csv_v2.zip` (20 302 272 byte, md5:17b8c13425c71b17f87a540178d45d9c) | `bb25dffa9792d65650428a81bcdf046b48e25ca130dd72fb8af137e65bfe49dd` |
| `Map17_070Ma.csv` | samma | `7bb4d97dbbccfcc2f5f0e3f4f9548fcfd88e8460b0861027a491e7a6d8fa8f95` |

Rotationsfilens första rader säger: "PALEOMAP Plate Model m15g60_v2d3", "for use with Plate Polygons ContOCeanPolyv10u_v2d3",
"by CR Scotese 02/01/2016". Zip-filerna ligger i `work/paleodem/nedladdat/` (inte i git), hämtade 2026-11-19 från Zenodo; md5 lika med
postens. Hämta igen: `curl -L -o <namn> "https://zenodo.org/api/records/5460860/files/<namn>/content"`.
