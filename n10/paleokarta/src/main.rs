//! paleokarta: en plats i dag till sitt läge för T miljoner år sedan (PALEOMAP), och höjden där och västerut i PaleoDEM-kartan.
//!
//! Användning: `paleokarta --lat 47.6377 --lon -106.5699 [--tid 66] [--dem data/Map16_065Ma.csv] [--vaster 30] [--oster 30]
//! [--platta 101]`. Utskriften: plattan, läget då, höjden där och höjden längs bredgraden väster- och österut.
//! Utan `--platta` väljs plattan ur polygonerna (`data/PALEOMAP_PlatePolygons.gpml`); finns platsen i flera eller ingen avbryts
//! körningen. Allt läses ur `data/` (källor och licens i `data/KALLA.md`).
use std::path::PathBuf;

use paleokarta::{Dem, Rotationer, las_polygoner, plattor_vid};

fn varde(a: &[String], namn: &str) -> Result<Option<String>, String> {
    match a.iter().position(|x| x == namn) {
        None => Ok(None),
        Some(i) => a.get(i + 1).cloned().map(Some).ok_or(format!("{namn} ska följas av ett värde")),
    }
}

fn tal(a: &[String], namn: &str) -> Result<Option<f64>, String> {
    varde(a, namn)?.map(|s| s.parse::<f64>().map_err(|_| format!("{namn}: {s} är inget tal"))).transpose()
}

fn las(fil: &PathBuf) -> Result<String, String> {
    std::fs::read_to_string(fil).map_err(|e| format!("{}: {e}", fil.display()))
}

fn kor(a: &[String]) -> Result<(), String> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
    let lat = tal(a, "--lat")?.ok_or("--lat saknas")?;
    let lon = tal(a, "--lon")?.ok_or("--lon saknas")?;
    let t = tal(a, "--tid")?.unwrap_or(66.0);
    let vaster = tal(a, "--vaster")?.unwrap_or(30.0) as i32;
    let dem_fil = varde(a, "--dem")?.map(PathBuf::from).unwrap_or(data.join("Map16_065Ma.csv"));
    let rot = Rotationer::las(&las(&data.join("PALEOMAP_PlateModel.rot"))?)?;
    let platta = match tal(a, "--platta")? {
        Some(p) => p as u32,
        None => {
            let poly = las_polygoner(&las(&data.join("PALEOMAP_PlatePolygons.gpml"))?)?;
            match plattor_vid(&poly, lat, lon, t)[..] {
                [p] => p,
                [] => return Err(format!("({lat}, {lon}) ligger inte i någon platta som finns vid {t} miljoner år; ange --platta")),
                ref flera => return Err(format!("({lat}, {lon}) ligger i flera plattor: {flera:?}; ange --platta")),
            }
        }
    };
    let (pla, plo) = rot.flytta(platta, lat, lon, t)?;
    let dem = Dem::las(&las(&dem_fil)?)?;
    println!("platsen i dag: {lat:.4}° N, {lon:.4}° Ö; platta {platta}; kedjan till jordens fasta ram {:?}", rot.kedja(platta, t));
    println!("läget för {t} miljoner år sedan (PALEOMAP): {pla:.3}° N, {plo:.3}° Ö");
    println!("höjden där i {}: {:.0} m (bilinjärt mellan de fyra närmaste punkterna)", dem_fil.display(), dem.hojd(pla, plo));
    // Avståndet längs bredgraden: en grad längdgrad är 2π · R · cos(bredgraden) / 360, med jordens medelradie R = 6 371 km
    // (IUGG:s medelradie, eget val av radie; kartan har en punkt per grad, så avståndet är aldrig noggrannare än en grad).
    let km_per_grad = 2.0 * std::f64::consts::PI * 6371.0 * pla.round().to_radians().cos() / 360.0;
    let oster = tal(a, "--oster")?.unwrap_or(vaster as f64) as i32;
    println!("längs bredgraden {:.0}° (hela grader, {km_per_grad:.0} km per grad; högst och lägst inom ±2° i nord–syd):", pla.round());
    println!("grader (− väster, + öster)  km från platsen  längdgrad  höjd (m)  högst ±2° (m)  lägst ±2° (m)");
    let (a, o) = (pla.round() as i32, plo.round() as i32);
    for d in -vaster..=oster {
        let lo = o + d;
        let band: Vec<f64> = (-2..=2).map(|da| dem.punkt(a + da, lo)).collect();
        let (hogst, lagst) = (band.iter().cloned().fold(f64::MIN, f64::max), band.iter().cloned().fold(f64::MAX, f64::min));
        println!("{d:>26}  {:>15.0}  {:>9}  {:>8.0}  {:>13.0}  {:>13.0}", d.abs() as f64 * km_per_grad, (lo + 180).rem_euclid(360) - 180, dem.punkt(a, lo), hogst, lagst);
    }
    Ok(())
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if let Err(e) = kor(&a) {
        eprintln!("paleokarta: {e}");
        std::process::exit(2);
    }
}
