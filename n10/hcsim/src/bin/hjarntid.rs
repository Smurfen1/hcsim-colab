//! hjarntid: mäter tiden per hjärnsteg för olika hjärnstorlekar på en kärna (Alt 1679, Håkans beslut 2026-11-22).
//! Användning: hjarntid [--hjarnor 6000] [--varv 500] [--omgangar 5] [--storlek S,Z,A]...
//! Varje omgång kör `hjarnor · varv` steg; raden visar den snabbaste och medianen i nanosekunder per steg.
//! Raden "Hj" är simuleringens egen hjärna (`hjarna::Hj`) vid standardstorleken, som jämförelse.

use std::time::Instant;

use hcsim::hjarna::Hj;
use hcsim::hjarnstorlek::{HjS, Storlek};
use hcsim::Rng;

const POOL: usize = 1024;

fn sinnespool(g: &mut Rng, s: usize) -> Vec<Vec<f64>> {
    (0..POOL).map(|_| (0..s).map(|_| 2.0 * g.f() - 1.0).collect()).collect()
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let arg: Vec<String> = std::env::args().skip(1).collect();
    let (mut hjarnor, mut varv, mut omgangar) = (6000usize, 500usize, 5usize);
    let mut storlekar: Vec<Storlek> = Vec::new();
    let mut i = 0;
    while i < arg.len() {
        let v = arg.get(i + 1).unwrap_or_else(|| panic!("värde saknas efter {}", arg[i]));
        match arg[i].as_str() {
            "--hjarnor" => hjarnor = v.parse().expect("--hjarnor"),
            "--varv" => varv = v.parse().expect("--varv"),
            "--omgangar" => omgangar = v.parse().expect("--omgangar"),
            "--storlek" => {
                let d: Vec<usize> = v.split(',').map(|x| x.parse().expect("--storlek S,Z,A")).collect();
                assert_eq!(d.len(), 3, "--storlek S,Z,A");
                storlekar.push(Storlek { s: d[0], z: d[1], a: d[2] });
            }
            x => panic!("okänt val {x}"),
        }
        i += 2;
    }
    assert!(hjarnor > 0 && varv > 0 && omgangar > 0);
    if storlekar.is_empty() {
        storlekar = vec![
            Storlek::STANDARD,
            Storlek { s: 8, z: 2, a: 5 },
            Storlek { s: 6, z: 2, a: 4 },
            Storlek { s: 19, z: 2, a: 9 },
            Storlek { s: 19, z: 3, a: 9 },
            Storlek { s: 19, z: 5, a: 9 },
            Storlek { s: 19, z: 6, a: 9 },
            Storlek { s: 19, z: 8, a: 9 },
        ];
    }
    let steg = (hjarnor * varv) as f64;
    println!("hjarna\tS\tZ\tA\tvikter\tns_per_steg_min\tns_per_steg_median\tsteg_per_omgang\tkontrollsumma");
    // Simuleringens egen hjärna vid standardstorleken.
    {
        let mut g = Rng::new(1679);
        let mut hj: Vec<Hj> = (0..hjarnor).map(|_| Hj::slump(&mut g)).collect();
        let pool = sinnespool(&mut g, Storlek::STANDARD.s);
        let mut tider = Vec::new();
        let mut summa = 0usize;
        for _ in 0..omgangar {
            let t0 = Instant::now();
            for v in 0..varv {
                for (n, h) in hj.iter_mut().enumerate() {
                    summa += h.valj(&pool[(n + 31 * v) % POOL], true, 0);
                }
            }
            tider.push(t0.elapsed().as_secs_f64() * 1e9 / steg);
        }
        let min = tider.iter().cloned().fold(f64::INFINITY, f64::min);
        let st = Storlek::STANDARD;
        println!("Hj\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{}\t{}", st.s, st.z, st.a, st.n_gen(), min, median(&mut tider), hjarnor * varv, summa);
    }
    for st in storlekar {
        let mut g = Rng::new(1679);
        let mut hj: Vec<HjS> = (0..hjarnor).map(|_| HjS::slump(st, &mut g)).collect();
        let pool = sinnespool(&mut g, st.s);
        let mut tider = Vec::new();
        let mut summa = 0usize;
        for _ in 0..omgangar {
            let t0 = Instant::now();
            for v in 0..varv {
                for (n, h) in hj.iter_mut().enumerate() {
                    summa += h.valj(&pool[(n + 31 * v) % POOL], true);
                }
            }
            tider.push(t0.elapsed().as_secs_f64() * 1e9 / steg);
        }
        let min = tider.iter().cloned().fold(f64::INFINITY, f64::min);
        println!("HjS\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{}\t{}", st.s, st.z, st.a, st.n_gen(), min, median(&mut tider), hjarnor * varv, summa);
    }
}
