//! Paleokartan: flyttar en plats på jorden i dag till sitt läge för miljoner år sedan med PALEOMAP-modellens rotationer av
//! kontinenterna (Scotese 2016, `data/PALEOMAP_PlateModel.rot`), och läser höjden där i PaleoDEM-kartan (Scotese och Wright
//! 2018, `data/Map16_065Ma.csv`). Håkan 2026-11-19: "Hämta kartmodellens rotationer av kontinenterna så allt blir exakt".
//! Källorna och licensen (CC BY 4.0) står i `data/KALLA.md` och i `n10/dinobiom/kallor-paleodem.md`.
//!
//! Räkningen följer GPlates regler för rotationsfiler: en rad är `platta tid bredgrad längdgrad vinkel fast_platta`, där
//! (bredgrad, längdgrad) är rotationsaxelns pol och vinkeln är i grader (högerhandsregeln). Mellan två rader för samma par
//! av plattor interpoleras rotationen (sfärisk interpolation av kvaternioner). Rotationen mot jordens fasta ram (platta 0)
//! är kedjan av fasta plattor: R(p→0) = R(fast→0) · R(p→fast).

use std::collections::HashMap;

/// En kvaternion för en rotation (w + xi + yj + zk, enhetslängd).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kvat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const ETT: Kvat = Kvat { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

/// Enhetsvektorn för bredgrad och längdgrad (grader): x mot 0° N 0° Ö, z mot nordpolen.
pub fn vektor(lat: f64, lon: f64) -> [f64; 3] {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    [la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()]
}

/// Bredgrad och längdgrad (grader, längdgraden −180 till 180) för en vektor.
pub fn latlon(v: [f64; 3]) -> (f64, f64) {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    ((v[2] / n).clamp(-1.0, 1.0).asin().to_degrees(), v[1].atan2(v[0]).to_degrees())
}

impl Kvat {
    /// Rotationen `vinkel` grader runt polen (lat, lon), moturs sett från polen utifrån (högerhandsregeln).
    pub fn pol(lat: f64, lon: f64, vinkel: f64) -> Kvat {
        let a = vektor(lat, lon);
        let h = vinkel.to_radians() / 2.0;
        Kvat { w: h.cos(), x: h.sin() * a[0], y: h.sin() * a[1], z: h.sin() * a[2] }
    }

    /// Först `b`, sedan `self`.
    pub fn efter(self, b: Kvat) -> Kvat {
        let a = self;
        Kvat {
            w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
            x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
            y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
            z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        }
    }

    pub fn rotera(self, v: [f64; 3]) -> [f64; 3] {
        let p = Kvat { w: 0.0, x: v[0], y: v[1], z: v[2] };
        let k = Kvat { w: self.w, x: -self.x, y: -self.y, z: -self.z };
        let r = self.efter(p).efter(k);
        [r.x, r.y, r.z]
    }

    fn punkt(self, b: Kvat) -> f64 {
        self.w * b.w + self.x * b.x + self.y * b.y + self.z * b.z
    }

    /// Sfärisk interpolation från `self` (f = 0) till `b` (f = 1), den kortaste vägen.
    pub fn slerp(self, b: Kvat, f: f64) -> Kvat {
        let mut d = self.punkt(b);
        let b = if d < 0.0 {
            d = -d;
            Kvat { w: -b.w, x: -b.x, y: -b.y, z: -b.z }
        } else {
            b
        };
        let (s0, s1) = if d > 0.999_999_999 {
            (1.0 - f, f)
        } else {
            let o = d.acos();
            (((1.0 - f) * o).sin() / o.sin(), (f * o).sin() / o.sin())
        };
        let q = Kvat { w: s0 * self.w + s1 * b.w, x: s0 * self.x + s1 * b.x, y: s0 * self.y + s1 * b.y, z: s0 * self.z + s1 * b.z };
        let n = q.punkt(q).sqrt();
        Kvat { w: q.w / n, x: q.x / n, y: q.y / n, z: q.z / n }
    }
}

/// En rad i rotationsfilen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rad {
    pub platta: u32,
    pub tid: f64,
    pub lat: f64,
    pub lon: f64,
    pub vinkel: f64,
    pub fast: u32,
}

pub struct Rotationer {
    /// Raderna per rörlig platta, i filens ordning.
    pub rader: HashMap<u32, Vec<Rad>>,
}

impl Rotationer {
    /// Läser en rotationsfil i GPlates format (PLATES4). Rader som inte börjar med sex tal, och platta 999 (kommentarer), hoppas över.
    pub fn las(text: &str) -> Result<Rotationer, String> {
        let mut rader: HashMap<u32, Vec<Rad>> = HashMap::new();
        for (n, l) in text.lines().enumerate() {
            let f: Vec<&str> = l.split('!').next().unwrap_or("").split_whitespace().collect();
            if f.len() < 6 {
                continue;
            }
            let tal: Option<Vec<f64>> = f[..6].iter().map(|x| x.parse::<f64>().ok()).collect();
            let Some(t) = tal else { continue };
            let r = Rad { platta: t[0] as u32, tid: t[1], lat: t[2], lon: t[3], vinkel: t[4], fast: t[5] as u32 };
            if r.platta == 999 {
                continue;
            }
            if !(-90.0..=90.0).contains(&r.lat) {
                return Err(format!("rad {}: polens bredgrad {} utanför −90..90", n + 1, r.lat));
            }
            rader.entry(r.platta).or_default().push(r);
        }
        if rader.is_empty() {
            return Err("inga rotationer i filen".into());
        }
        Ok(Rotationer { rader })
    }

    /// Rotationen av plattan `p` mot den fasta plattan i det radpar som omfattar tiden `t`, och den fasta plattan.
    pub fn relativ(&self, p: u32, t: f64) -> Option<(Kvat, u32)> {
        let r = self.rader.get(&p)?;
        for w in r.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a.fast != b.fast || !(a.tid <= t && t <= b.tid) {
                continue;
            }
            let (qa, qb) = (Kvat::pol(a.lat, a.lon, a.vinkel), Kvat::pol(b.lat, b.lon, b.vinkel));
            let f = if b.tid > a.tid { (t - a.tid) / (b.tid - a.tid) } else { 0.0 };
            return Some((qa.slerp(qb, f), a.fast));
        }
        None
    }

    /// Rotationen av plattan `p` mot jordens fasta ram (platta 0) vid tiden `t` (miljoner år), genom kedjan av fasta plattor.
    pub fn total(&self, p: u32, t: f64) -> Result<Kvat, String> {
        let mut q = ETT;
        let mut nu = p;
        let mut kedja = vec![p];
        while nu != 0 {
            let (r, fast) = self.relativ(nu, t).ok_or_else(|| format!("platta {nu} saknar rotation vid {t} miljoner år (kedjan {kedja:?})"))?;
            q = r.efter(q);
            nu = fast;
            kedja.push(nu);
            if kedja.len() > 64 {
                return Err(format!("kedjan av plattor slutar inte: {kedja:?}"));
            }
        }
        Ok(q)
    }

    /// Kedjan av fasta plattor från `p` till 0 vid tiden `t`.
    pub fn kedja(&self, p: u32, t: f64) -> Vec<u32> {
        let mut k = vec![p];
        let mut nu = p;
        while nu != 0 && k.len() < 64 {
            match self.relativ(nu, t) {
                Some((_, f)) => {
                    k.push(f);
                    nu = f;
                }
                None => break,
            }
        }
        k
    }

    /// Läget (bredgrad, längdgrad) vid tiden `t` för platsen (lat, lon) i dag på plattan `p`.
    pub fn flytta(&self, p: u32, lat: f64, lon: f64, t: f64) -> Result<(f64, f64), String> {
        Ok(latlon(self.total(p, t)?.rotera(vektor(lat, lon))))
    }
}

/// En platta i polygonfilen: numret, tiden då den finns (från `borjan` till `slut` miljoner år; borjan ≥ slut) och ytterkanten.
pub struct Polygon {
    pub platta: u32,
    pub borjan: f64,
    pub slut: f64,
    /// Hörnen som (bredgrad, längdgrad).
    pub horn: Vec<(f64, f64)>,
}

fn mellan<'a>(s: &'a str, fore: &str, efter: &str) -> Option<&'a str> {
    let i = s.find(fore)? + fore.len();
    let j = s[i..].find(efter)? + i;
    Some(&s[i..j])
}

fn tidpunkt(s: &str) -> Option<f64> {
    let v = mellan(s, "<gml:timePosition", "</gml:timePosition>")?;
    let v = v[v.find('>')? + 1..].trim();
    if v.ends_with("distantPast") {
        Some(f64::INFINITY)
    } else if v.ends_with("distantFuture") {
        Some(f64::NEG_INFINITY)
    } else {
        v.parse().ok()
    }
}

/// Läser plattornas polygoner ur en GPML-fil (GPlates format; ytterkanten i `gml:exterior`, hörnen som "bredgrad längdgrad").
pub fn las_polygoner(text: &str) -> Result<Vec<Polygon>, String> {
    let mut ut = Vec::new();
    for f in text.split("<gml:featureMember>").skip(1) {
        let platta = mellan(f, "<gpml:reconstructionPlateId>", "</gpml:reconstructionPlateId>")
            .and_then(|s| mellan(s, "<gpml:value>", "</gpml:value>"))
            .and_then(|s| s.trim().parse::<u32>().ok())
            .ok_or("en polygon saknar plattans nummer")?;
        let borjan = mellan(f, "<gml:begin>", "</gml:begin>").and_then(tidpunkt).unwrap_or(f64::INFINITY);
        let slut = mellan(f, "<gml:end>", "</gml:end>").and_then(tidpunkt).unwrap_or(f64::NEG_INFINITY);
        for del in f.split("<gml:exterior>").skip(1) {
            let lista = mellan(del, "<gml:posList", "</gml:posList>").ok_or("ytterkant utan hörn")?;
            let tal: Vec<f64> = lista[lista.find('>').ok_or("trasig posList")? + 1..].split_whitespace().map(|x| x.parse::<f64>()).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
            if tal.len() < 6 || tal.len() % 2 != 0 {
                return Err(format!("platta {platta}: {} tal i en ytterkant", tal.len()));
            }
            ut.push(Polygon { platta, borjan, slut, horn: tal.chunks(2).map(|c| (c[0], c[1])).collect() });
        }
    }
    Ok(ut)
}

impl Polygon {
    pub fn finns(&self, t: f64) -> bool {
        self.borjan >= t && t >= self.slut
    }

    /// Ligger (lat, lon) innanför ytterkanten? Strålprovet i planet (längdgrad, bredgrad). Hörnens längdgrader görs
    /// sammanhängande längs kanten (ett hopp över datumlinjen blir ett steg på mindre än 180°), och punkten prövas med sin
    /// längdgrad och ±360°, så att en polygon över datumlinjen räknas rätt. En polygon runt en pol räknas inte rätt.
    pub fn innehaller(&self, lat: f64, lon: f64) -> bool {
        let mut lo: Vec<f64> = Vec::with_capacity(self.horn.len());
        for &(_, l) in &self.horn {
            let l = match lo.last() {
                Some(&f) => l + 360.0 * ((f - l) / 360.0).round(),
                None => l,
            };
            lo.push(l);
        }
        let n = self.horn.len();
        [lon - 360.0, lon, lon + 360.0].into_iter().any(|plon| {
            let mut inne = false;
            for i in 0..n {
                let (la1, lo1) = (self.horn[i].0, lo[i]);
                let (la2, lo2) = (self.horn[(i + 1) % n].0, lo[(i + 1) % n]);
                if (la1 > lat) != (la2 > lat) {
                    let x = lo1 + (lat - la1) / (la2 - la1) * (lo2 - lo1);
                    if plon < x {
                        inne = !inne;
                    }
                }
            }
            inne
        })
    }
}

/// Plattorna vars polygon innehåller platsen (lat, lon) i dag och som finns vid tiden `t`, utan dubbletter.
pub fn plattor_vid(poly: &[Polygon], lat: f64, lon: f64, t: f64) -> Vec<u32> {
    let mut ut: Vec<u32> = poly.iter().filter(|p| p.finns(t) && p.finns(0.0) && p.innehaller(lat, lon)).map(|p| p.platta).collect();
    ut.sort();
    ut.dedup();
    ut
}

/// En PaleoDEM-karta i textformat (`lon,lat,elev`, en punkt per hel grad, höjden i meter).
pub struct Dem {
    hojd: HashMap<(i32, i32), f64>,
}

impl Dem {
    pub fn las(text: &str) -> Result<Dem, String> {
        let mut hojd = HashMap::new();
        for (n, l) in text.lines().enumerate() {
            if l.starts_with('#') || l.trim().is_empty() {
                continue;
            }
            let f: Vec<f64> = l.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|e| format!("rad {}: {e}", n + 1))?;
            if f.len() != 3 || f[0].fract() != 0.0 || f[1].fract() != 0.0 {
                return Err(format!("rad {}: väntade lon,lat,höjd i hela grader", n + 1));
            }
            hojd.insert((f[0] as i32, f[1] as i32), f[2]);
        }
        if hojd.len() != 361 * 181 {
            return Err(format!("{} punkter, väntade 361 · 181", hojd.len()));
        }
        Ok(Dem { hojd })
    }

    /// Höjden i en hel grad (längdgraden räknas om till −180..180).
    pub fn punkt(&self, lat: i32, lon: i32) -> f64 {
        let lon = (lon + 180).rem_euclid(360) - 180;
        self.hojd[&(lon, lat.clamp(-90, 90))]
    }

    /// Höjden i (lat, lon), bilinjärt mellan de fyra närmaste punkterna.
    pub fn hojd(&self, lat: f64, lon: f64) -> f64 {
        let (la0, lo0) = (lat.floor(), lon.floor());
        let (fa, fo) = (lat - la0, lon - lo0);
        let (a, o) = (la0 as i32, lo0 as i32);
        let h = |da: i32, dob: i32| self.punkt((a + da).min(90), o + dob);
        (1.0 - fa) * ((1.0 - fo) * h(0, 0) + fo * h(0, 1)) + fa * ((1.0 - fo) * h(1, 0) + fo * h(1, 1))
    }
}

#[cfg(test)]
mod prov {
    use super::*;

    fn nara(a: (f64, f64), b: (f64, f64), tol: f64) -> bool {
        let d = |x: f64, y: f64| ((x - y + 540.0).rem_euclid(360.0) - 180.0).abs();
        (a.0 - b.0).abs() < tol && d(a.1, b.1) < tol
    }

    /// Räknesättet prövat mot fall som går att räkna för hand: 90° runt nordpolen flyttar 0° N 0° Ö till 0° N 90° Ö (högerhand,
    /// moturs sett ovanifrån); polen själv står still; halva tiden mellan 0° och 40° runt samma pol ger 20°.
    #[test]
    fn rotationer_som_gar_att_rakna_for_hand() {
        let q = Kvat::pol(90.0, 0.0, 90.0);
        assert!(nara(latlon(q.rotera(vektor(0.0, 0.0))), (0.0, 90.0), 1e-9));
        assert!(nara(latlon(Kvat::pol(30.0, 40.0, 77.0).rotera(vektor(30.0, 40.0))), (30.0, 40.0), 1e-9));
        let fil = "101 0.0 0.0 0.0 0.0 000 !\n101 10.0 90.0 0.0 40.0 000 !\n";
        let r = Rotationer::las(fil).unwrap();
        assert!(nara(r.flytta(101, 0.0, 0.0, 5.0).unwrap(), (0.0, 20.0), 1e-9));
        // Kedjans ordning prövas med två olika poler: först 90° runt (0° N, 0° Ö) för 102 mot 101, sedan 90° runt nordpolen
        // för 101 mot 0. (0° N, 90° Ö) roteras först till nordpolen, som står still i den andra rotationen. Vid polen saknar
        // längdgraden betydelse, så riktningarna jämförs som vektorer.
        let fil = "101 0 0 0 0 000 !\n101 100 90 0 90 000 !\n102 0 0 0 0 101 !\n102 100 0 0 90 101 !\n";
        let r = Rotationer::las(fil).unwrap();
        let v = r.total(102, 100.0).unwrap().rotera(vektor(0.0, 90.0));
        assert!((v[2] - 1.0).abs() < 1e-9, "punkten hamnade i {:?}, inte på nordpolen", latlon(v));
        // I omvänd ordning hade punkten först gått till (0° N, 180° Ö) och stått kvar där.
        assert_eq!(r.kedja(102, 50.0), vec![102, 101, 0]);
    }

    /// Ett radpar med olika fasta plattor (en övergång) interpoleras inte; tiden efter övergången följer den nya fasta plattan.
    #[test]
    fn overgang_mellan_fasta_plattor() {
        let fil = "101 0 0 0 0 000 !\n101 50 90 0 50 000 !\n101 50 90 0 0 201 !\n101 100 90 0 10 201 !\n201 0 0 0 0 000 !\n201 100 90 0 100 000 !\n";
        let r = Rotationer::las(fil).unwrap();
        assert_eq!(r.kedja(101, 25.0), vec![101, 0]);
        assert_eq!(r.kedja(101, 75.0), vec![101, 201, 0]);
        // 75 miljoner år: 101 mot 201 är 5°, 201 mot 0 är 75°: totalt 80° runt nordpolen.
        assert!(nara(r.flytta(101, 0.0, 0.0, 75.0).unwrap(), (0.0, 80.0), 1e-9));
        // Två rader i följd med olika fasta plattor och utan gemensam tid: mellan dem finns ingen rotation, och rotationer mot
        // olika plattor får inte blandas (då hade 101 vid 75 miljoner år fått 30° runt nordpolen mot en blandning av 0 och 201).
        let glapp = "101 0 0 0 0 000 !\n101 50 90 0 50 000 !\n101 100 90 0 10 201 !\n201 0 0 0 0 000 !\n201 100 90 0 100 000 !\n";
        let r = Rotationer::las(glapp).unwrap();
        assert!(r.total(101, 75.0).is_err(), "rotationen blandade två fasta plattor: {:?}", r.flytta(101, 0.0, 0.0, 75.0));
        assert!(nara(r.flytta(101, 0.0, 0.0, 25.0).unwrap(), (0.0, 25.0), 1e-9));
    }

    #[test]
    fn polygon_och_datumlinjen() {
        let p = Polygon { platta: 1, borjan: 100.0, slut: f64::NEG_INFINITY, horn: vec![(0.0, 170.0), (0.0, -170.0), (10.0, -170.0), (10.0, 170.0)] };
        assert!(p.innehaller(5.0, 179.0) && p.innehaller(5.0, -179.0));
        assert!(!p.innehaller(5.0, 160.0) && !p.innehaller(15.0, 179.0));
        assert!(p.finns(66.0) && !p.finns(150.0));
        // En polygon i Asien (60–90° Ö) innehåller inte en punkt i Montana (−106,6°): så räknade första versionen fel
        // (2026-11-19, platta 402), eftersom hörnen flyttades var för sig mot punktens längdgrad.
        let asien = Polygon { platta: 402, borjan: 420.0, slut: f64::NEG_INFINITY, horn: vec![(40.0, 60.0), (40.0, 90.0), (65.0, 90.0), (65.0, 60.0)] };
        assert!(!asien.innehaller(47.6, -106.6) && asien.innehaller(50.0, 75.0) && !asien.innehaller(50.0, 95.0));
    }
}
