//! Jämförelse med en oberoende räknare: GPlates webbtjänst (gws.gplates.org, modellen PALEOMAP), svaren sparade i
//! `tests/gplates/gws-<tid>.json` (frågorna i `fragor.tsv`, hämtade enligt `hamtad.txt`). Tolv platser på olika plattor och sju
//! tider. Koden väljer plattan ur polygonerna och flyttar platsen; läget ska vara GPlates läge inom 0,001° (GPlates svarar med
//! fyra decimaler). Gränsen sattes innan provet kördes första gången.
use paleokarta::{Rotationer, las_polygoner, plattor_vid};

const GRANS: f64 = 0.001;

fn fil(namn: &str) -> String {
    std::fs::read_to_string(format!("{}/{namn}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{namn}: {e}"))
}

/// Talen i GeoJSON-svarets lista "coordinates", parvis (längdgrad, bredgrad).
fn koordinater(json: &str) -> Vec<(f64, f64)> {
    let i = json.find("\"coordinates\"").expect("svaret saknar coordinates");
    let tal: Vec<f64> = json[i + 13..].split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e')).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
    tal.chunks(2).map(|c| (c[0], c[1])).collect()
}

fn punkter() -> Vec<(String, f64, f64)> {
    fil("tests/gplates/punkter.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[0].to_string(), f[1].parse().unwrap(), f[2].parse().unwrap())
        })
        .collect()
}

fn skillnad(a: f64, b: f64) -> f64 {
    ((a - b + 540.0).rem_euclid(360.0) - 180.0).abs()
}

/// Jämför alla punkter vid alla tider; ger (antal lika, lista med avvikelser).
pub fn jamfor(rot: &Rotationer, plattor: &dyn Fn(f64, f64, f64) -> Vec<u32>) -> (usize, Vec<String>) {
    let p = punkter();
    let (mut lika, mut fel) = (0, Vec::new());
    for t in [0.0, 30.0, 66.0, 69.0, 100.0, 150.0, 250.0] {
        let gws = koordinater(&fil(&format!("tests/gplates/gws-{t}.json")));
        assert_eq!(gws.len(), p.len(), "tid {t}: {} punkter i svaret", gws.len());
        for ((namn, lon, lat), (glon, glat)) in p.iter().zip(gws) {
            let pl = plattor(*lat, *lon, t);
            // GPlates svarar 999,99 när platsen inte har något läge vid tiden (plattans polygon finns inte då, till exempel
            // Tokyo vid 250 miljoner år: platta 610 finns från 245). Då ska koden heller inte hitta någon platta.
            if glat > 999.0 && glon > 999.0 {
                if pl.is_empty() {
                    lika += 1;
                } else {
                    fel.push(format!("{namn}, {t} Ma: GPlates ger inget läge, koden hittar plattorna {pl:?}"));
                }
                continue;
            }
            let [platta] = pl[..] else {
                fel.push(format!("{namn}, {t} Ma: plattorna {pl:?}"));
                continue;
            };
            match rot.flytta(platta, *lat, *lon, t) {
                Ok((la, lo)) if (la - glat).abs() < GRANS && (glat.abs() > 89.9 || skillnad(lo, glon) < GRANS) => lika += 1,
                Ok((la, lo)) => fel.push(format!("{namn}, {t} Ma, platta {platta}: {la:.4}, {lo:.4} mot GPlates {glat}, {glon}")),
                Err(e) => fel.push(format!("{namn}, {t} Ma: {e}")),
            }
        }
    }
    (lika, fel)
}

#[test]
fn samma_lage_som_gplates_for_alla_punkter_och_tider() {
    let rot = Rotationer::las(&fil("data/PALEOMAP_PlateModel.rot")).unwrap();
    let poly = las_polygoner(&fil("data/PALEOMAP_PlatePolygons.gpml")).unwrap();
    let (lika, fel) = jamfor(&rot, &|la, lo, t| plattor_vid(&poly, la, lo, t));
    println!("lika som GPlates: {lika} av {}", lika + fel.len());
    assert!(fel.is_empty(), "{} avvikelser:\n{}", fel.len(), fel.join("\n"));
    assert_eq!(lika, 12 * 7);
}

/// Hell Creeks fem platser ligger på Nordamerikas platta (101) i polygonerna, som i PBDB (fältet gpl för alla 591 platser).
#[test]
fn hell_creek_ligger_pa_nordamerika() {
    let poly = las_polygoner(&fil("data/PALEOMAP_PlatePolygons.gpml")).unwrap();
    for (namn, lon, lat) in punkter().iter().take(5) {
        assert_eq!(plattor_vid(&poly, *lat, *lon, 66.0), vec![101], "{namn}");
    }
}
