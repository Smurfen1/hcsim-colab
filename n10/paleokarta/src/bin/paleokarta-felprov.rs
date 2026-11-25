// Felproven för paleokartan (PALEOMAP-rotationerna, 2026-11-19; samma program som n10/narbild/src/felprov.rs): sätter in ett fel i taget i en KOPIA av koden, kör provet
// som ska fånga felet och kräver att det blir rött; sedan återställs filen. Ett fäste som inte finns exakt en gång stoppar
// körningen innan något byggs. Användning: paleokarta-felprov <kopians n10/paleokarta> <mål för cargo> (LASMIG.md). Rör aldrig
// arkivet: kör bara mot en kopia.
use std::path::Path;
use std::process::Command;

struct Fel {
    namn: &'static str,
    fil: &'static str,
    fore: &'static str,
    efter: &'static str,
    /// Paketets mapp (relativt kopians rot) och provet som ska bli rött.
    paket: &'static str,
    prov: &'static str,
}

const FEL: &[Fel] = &[
    Fel { namn: "vinkeln åt fel håll", fil: "src/lib.rs", fore: "let h = vinkel.to_radians() / 2.0;", efter: "let h = -vinkel.to_radians() / 2.0;", paket: ".", prov: "samma_lage_som_gplates" },
    Fel { namn: "kedjan i omvänd ordning", fil: "src/lib.rs", fore: "            q = r.efter(q);\n", efter: "            q = q.efter(r);\n", paket: ".", prov: "samma_lage_som_gplates" },
    Fel { namn: "ingen interpolation mellan raderna", fil: "src/lib.rs", fore: "let f = if b.tid > a.tid { (t - a.tid) / (b.tid - a.tid) } else { 0.0 };", efter: "let f = 0.0;", paket: ".", prov: "samma_lage_som_gplates" },
    Fel { namn: "polygonens hörn läses som längdgrad, bredgrad", fil: "src/lib.rs", fore: "horn: tal.chunks(2).map(|c| (c[0], c[1])).collect()", efter: "horn: tal.chunks(2).map(|c| (c[1], c[0])).collect()", paket: ".", prov: "hell_creek_ligger_pa_nordamerika" },
    Fel { namn: "polygonens tid prövas inte", fil: "src/lib.rs", fore: ".filter(|p| p.finns(t) && p.finns(0.0) && p.innehaller(lat, lon))", efter: ".filter(|p| p.innehaller(lat, lon))", paket: ".", prov: "samma_lage_som_gplates" },
    Fel { namn: "radpar över en övergång mellan fasta plattor", fil: "src/lib.rs", fore: "if a.fast != b.fast || !(a.tid <= t && t <= b.tid) {", efter: "if !(a.tid <= t && t <= b.tid) {", paket: ".", prov: "overgang_mellan_fasta_plattor" },
];

fn kor_prov(rot: &Path, mal: &str, paket: &str, prov: &str) -> bool {
    let s = Command::new(format!("{}/.cargo/bin/cargo", std::env::var("HOME").unwrap()))
        .args(["test", "--release", "-j", "4", prov])
        .env("CARGO_TARGET_DIR", mal)
        .current_dir(rot.join(paket))
        .output()
        .expect("cargo kunde inte startas");
    let text = String::from_utf8_lossy(&s.stdout);
    // Provet ska ha körts (inte filtrerats bort): raden "test ... <prov> ..." ska finnas, eller ett byggfel.
    let kort = text.contains(prov) || !s.status.success();
    assert!(kort, "provet {prov} kördes inte i {paket}");
    s.status.success()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (rot, mal) = (Path::new(&a[1]), a[2].as_str());
    // Alla fästen prövas först.
    for f in FEL {
        let t = std::fs::read_to_string(rot.join(f.fil)).unwrap();
        assert_eq!(t.matches(f.fore).count(), 1, "felet \"{}\": fästet finns {} gånger i {}", f.namn, t.matches(f.fore).count(), f.fil);
    }
    // Utan fel ska varje prov vara grönt.
    for f in FEL {
        assert!(kor_prov(rot, mal, f.paket, f.prov), "provet {} är rött utan fel", f.prov);
    }
    println!("utan fel: alla {} prov gröna", FEL.len());
    let mut roda = 0;
    for f in FEL {
        let fil = rot.join(f.fil);
        let orig = std::fs::read_to_string(&fil).unwrap();
        std::fs::write(&fil, orig.replacen(f.fore, f.efter, 1)).unwrap();
        let gron = kor_prov(rot, mal, f.paket, f.prov);
        std::fs::write(&fil, &orig).unwrap();
        println!("{}: {} ({})", if gron { "GRÖNT, FELET SÅGS INTE" } else { "rött" }, f.namn, f.prov);
        roda += usize::from(!gron);
    }
    println!("{roda} av {} insatta fel gav rött prov", FEL.len());
    std::process::exit(if roda == FEL.len() { 0 } else { 1 });
}
