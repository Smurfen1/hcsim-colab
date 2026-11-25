//! Prov 1–12 för Alt 1401 (fåror som objekt) + Alt 1405 (Gransens ankarvärden).
//! Underlag: n10/docs/2026-11-21-lasunderlag-kartinterp.md (skrivet före bygget).

use std::path::PathBuf;
use std::process::Command;

use hckarta::{
    flodregler, karta_for_ar, karta_full_version, karta_med, kontrollsumma, kontrollsumma_med,
    levande_foror_nr, mittlinjer_aldre, nasta_andring, Kalla, Omrade, Period, Tidtabell, Vatten,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data")
}

fn kalla() -> Kalla {
    Kalla::las(&data()).expect("kunde inte läsa paleokarta/data")
}

fn tidtabell() -> Tidtabell {
    let text =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv"))
            .expect("kunde inte läsa tidtabell.tsv");
    Tidtabell::las(&text).expect("tidtabellen")
}

fn omrade() -> Omrade {
    Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 64,
        h: 48,
        ruta_m: 2000.0,
    }
}

fn temp_mapp(namn: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "hckarta-tid1401-{}-{}-{}",
        namn,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("temp-mapp");
    p
}

fn bin_hckarta() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_hckarta"))
}

fn sha256_fil(path: &std::path::Path) -> String {
    let ut = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum");
    assert!(
        ut.status.success(),
        "sha256sum misslyckades för {}",
        path.display()
    );
    let s = String::from_utf8_lossy(&ut.stdout);
    s.split_whitespace().next().unwrap().to_string()
}

fn las_fore_sha(version: &str, t: &str, fil: &str) -> String {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tid1401-fore.txt"),
    )
    .expect("tid1401-fore.txt saknas — räkna före-baseline först");
    let mark = format!("sha256 filer flodversion={version} t={t}:");
    let mut efter = false;
    for rad in text.lines() {
        if rad.contains(&mark) {
            efter = true;
            continue;
        }
        if efter {
            if rad.starts_with("===") || rad.starts_with("---") || rad.starts_with("sha256") {
                break;
            }
            let mut delar = rad.split_whitespace();
            if let (Some(hash), Some(namn)) = (delar.next(), delar.next()) {
                if namn == fil {
                    return hash.to_string();
                }
            }
        }
    }
    panic!("saknar sha256 för {fil} vid flodversion={version} t={t} i tid1401-fore.txt");
}

fn lika_vattentyp_mittlinje(a: &hckarta::Karta, b: &hckarta::Karta) {
    assert_eq!(a.vattentyp.varden, b.vattentyp.varden);
    assert_eq!(a.vatten_yta.varden, b.vatten_yta.varden);
}

/// Prov 1: gamla kartor orörda utom Gransen (t=500000 och 20000, v1 och v2).
#[test]
fn prov1_gamla_kartor_ororda_utom_gransen() {
    let hk = bin_hckarta();
    let tt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv");
    for version in ["1", "2"] {
        for t in ["500000", "20000"] {
            let mapp = temp_mapp(&format!("p1-v{version}-t{t}"));
            // Standardbytet (Alt 1566): provet gäller den gamla vägen; läge 1 läggs sist för de fyra valen, och en flagga som redan står tidigare gäller (programmet läser första förekomsten).
            let status = Command::new(&hk)
                .args([
                    "--lat0",
                    "47",
                    "--lon0",
                    "-108",
                    "--b",
                    "64",
                    "--h",
                    "48",
                    "--ruta-m",
                    "2000",
                    "--t",
                    t,
                    "--tidtabell",
                ])
                .arg(&tt)
                .args(["--fro", "181", "--flodversion", version, "--skriv"])
                .arg(&mapp)
                .args(["--flodversion", "1", "--tidsmodell", "1", "--flodriktning", "1", "--lutning", "1"]).status()
                .expect("kör hckarta");
            assert!(status.success(), "hckarta v={version} t={t}");
            let mut filer = vec![
                "hojd.f32",
                "vatten_yta.f32",
                "vattentyp.u8",
                "mark.f32",
                "vaxt.f32",
                "period.u8",
                "hojd.png",
                "strand.json",
                "karta.json",
            ];
            if version == "2" {
                // Alt 1371/1433: foror.tsv, karta.json (vall/växt + ny summa) ändras med flit;
                // jämför bara de gamla andelslagren här (fara/balte orörda av växtgrupper).
                filer.retain(|f| *f != "karta.json");
                filer.extend(["fara_andel.f32", "balte_andel.f32"]);
            }
            for fil in filer {
                let efter = sha256_fil(&mapp.join(fil));
                let fore = las_fore_sha(version, t, fil);
                assert_eq!(
                    efter, fore,
                    "fil {fil} flodversion={version} t={t} skiljer sig från före"
                );
            }
        }
    }
}

/// Prov 2: hcsim orört (slutsummor).
#[test]
fn prov2_hcsim_orort() {
    let hcsim = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim/target/release/hcsim");
    assert!(
        hcsim.exists(),
        "bygg hcsim release först: CARGO_TARGET_DIR=…/hcsim/target cargo build -j 4 --release i n10/hcsim"
    );
    let ut = Command::new(&hcsim)
        .args([
            "kor",
            "--lat0",
            "47",
            "--lon0",
            "-108",
            "--b",
            "64",
            "--h",
            "48",
            "--ruta-m",
            "2000",
            "--t",
            "500000",
            "--fro",
            "181",
            "--ar",
            "5",
            "--n0",
            "400",
        ])
        .output()
        .expect("hcsim");
    assert!(ut.status.success(), "hcsim misslyckades: {}", String::from_utf8_lossy(&ut.stderr));
    let stdout = String::from_utf8_lossy(&ut.stdout);
    // 5548 sedan hcsim 88c0a5f (BRAIN, Alt 1624: flodkanten får växter); kartans karta_summa oförändrad. Förut 4992.
    assert!(
        stdout.contains("\"slut_antal\":5548"),
        "slut_antal: {stdout}"
    );
    assert!(
        // djur_summa 4225976351498359620 sedan hcsim 88c0a5f (BRAIN, Alt 1624); förut 9527944887746424482.
        stdout.contains("\"djur_summa\":4225976351498359620"),
        "djur_summa: {stdout}"
    );
    assert!(
        stdout.contains("\"karta_summa\":9828389463751256610"),
        "karta_summa: {stdout}"
    );
}

/// Prov 3: Vid basen (inga loppbyten ännu) är karta_for_ar == karta_full_version (Aldre).
/// Efter Alt 1395 gäller inte längre t=500000/900000 (historiken har spelats upp).
#[test]
fn prov3_aldre_lika() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let basen = tt.last_tid("basen").unwrap_or(1_081_000.0);
    for fro in [181u64, 182, 183] {
        for version in [1u8, 2] {
            let (a, and_a) = karta_for_ar(&k, &tt, &omr, basen, fro, version, 1).unwrap();
            let (b, and_b) = karta_full_version(&k, &tt, &omr, basen, fro, version, 1).unwrap();
            assert_eq!(a.period.varden[0], Period::Aldre);
            lika_vattentyp_mittlinje(&a, &b);
            assert_eq!(kontrollsumma(&a), kontrollsumma(&b));
            assert_eq!(
                kontrollsumma_med(&a, and_a.as_ref()),
                kontrollsumma_med(&b, and_b.as_ref())
            );
            if version == 2 {
                let aa = and_a.as_ref().unwrap();
                let bb = and_b.as_ref().unwrap();
                assert_eq!(aa.fara_andel.varden, bb.fara_andel.varden);
                assert_eq!(aa.balte_andel.varden, bb.balte_andel.varden);
                assert_eq!(aa.foror, bb.foror);
            }
        }
    }
}

fn mittlinje_mask(linjer: &[Vec<(usize, usize)>], b: usize, n: usize) -> Vec<bool> {
    let mut m = vec![false; n];
    for path in linjer {
        for &(i, j) in path {
            m[i + j * b] = true;
        }
    }
    m
}

/// Prov 4: floderna står kvar — exakt två levande fåror (efter Alt 1395: aktuella loppen,
/// inte nödvändigtvis ursprungliga Aldre-mittlinjer).
#[test]
fn prov4_floderna_star_kvar() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for fro in [181u64, 182, 183] {
        for t in [20_000.0, 0.0] {
            for version in [1u8, 2] {
                let (kart, and) = karta_for_ar(&k, &tt, &omr, t, fro, version, 1).unwrap();
                let lev = levande_foror_nr(fro, kart.period.varden[0]);
                assert_eq!(lev.len(), 2, "fro={fro} t={t}");
                assert_ne!(lev[0], lev[1]);
                let (linjer, lev2) =
                    hckarta::mittlinjer_vid_ar(&k, &tt, &omr, t, fro, 1).unwrap();
                assert_eq!(lev, lev2);
                assert_eq!(linjer.len(), 2);
                if version == 1 {
                    for path in &linjer {
                        for &(i, j) in path {
                            let p = i + j * omr.b;
                            if kart.hojd.varden[p] < 0.0 {
                                continue;
                            }
                            assert_eq!(
                                kart.vattentyp.varden[p],
                                Vatten::Flodfara,
                                "levande mittlinje inte Flodfara fro={fro}"
                            );
                        }
                    }
                } else {
                    let a = and.as_ref().unwrap();
                    assert_eq!(a.foror.len(), 2);
                    for path in &linjer {
                        for &(i, j) in path {
                            let p = i + j * omr.b;
                            if kart.hojd.varden[p] < 0.0 {
                                continue;
                            }
                            assert!(
                                a.fara_andel.varden[p] > 0.0,
                                "levande mittlinje saknar fara_andel"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Prov 5: samma levande nummer i Ovre och Gransen; om inget loppbyte mellan
/// t=20000 och t=0+ε är mittlinjerna lika (Alt 1395: kan skilja om byte ligger emellan).
#[test]
fn prov5_gransen_behaller_ovres_foror() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for fro in [181u64, 182, 183] {
        assert_eq!(
            levande_foror_nr(fro, Period::Ovre),
            levande_foror_nr(fro, Period::Gransen)
        );
        let (linjer_o, _) = hckarta::mittlinjer_vid_ar(&k, &tt, &omr, 20_000.0, fro, 1).unwrap();
        let (linjer_g, _) = hckarta::mittlinjer_vid_ar(&k, &tt, &omr, 0.0, fro, 1).unwrap();
        assert_eq!(linjer_o.len(), 2);
        assert_eq!(linjer_g.len(), 2);
        // Bredd per fåra (v2) följer numret, oförändrat av loppbyten.
        for version in [1u8, 2] {
            let (_, and_o) = karta_for_ar(&k, &tt, &omr, 20_000.0, fro, version, 1).unwrap();
            let (_, and_g) = karta_for_ar(&k, &tt, &omr, 0.0, fro, version, 1).unwrap();
            if version == 2 {
                let ao = and_o.as_ref().unwrap();
                let ag = and_g.as_ref().unwrap();
                assert_eq!(ao.foror.len(), ag.foror.len());
                for (a, b) in ao.foror.iter().zip(ag.foror.iter()) {
                    assert!((a.bredd_m - b.bredd_m).abs() < 1e-12);
                    assert!((a.balte_m - b.balte_m).abs() < 1e-12);
                }
            }
        }
        // Om nasta_andring från 20000 är gränsen (0) och inte ett loppbyte: mittlinjer lika.
        if nasta_andring(&tt, 20_000.0, fro) == Some(0.0) {
            assert_eq!(linjer_o, linjer_g, "inga loppbyten 0..20000 → mittlinjer lika fro={fro}");
        }
    }
}

/// Prov 6: övergivna lopprutor (döda fåror + gamla nedströmmar) → Trask
/// där inget levande lopp går (Alt 1401 + 1395).
#[test]
fn prov6_overgivna_blir_trask() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for fro in [181u64, 182, 183] {
        for t in [20_000.0, 0.0] {
            for version in [1u8, 2] {
                let (kart, and) = karta_for_ar(&k, &tt, &omr, t, fro, version, 1).unwrap();
                let (linjer, _) = hckarta::mittlinjer_vid_ar(&k, &tt, &omr, t, fro, 1).unwrap();
                let mut levande_mitt = vec![false; omr.b * omr.h];
                for path in &linjer {
                    for &(i, j) in path {
                        levande_mitt[i + j * omr.b] = true;
                    }
                }
                // Minst några Trask-rutor ska finnas (övergivna loppdellar).
                let n_trask = kart
                    .vattentyp
                    .varden
                    .iter()
                    .filter(|&&v| v == Vatten::Trask)
                    .count();
                assert!(n_trask > 0, "förväntade Trask från övergivna fro={fro}");
                // Levande mittlinje får inte vara enbart Trask utan Flodfara/andel.
                if version == 1 {
                    for path in &linjer {
                        let mut n_fara = 0;
                        for &(i, j) in path {
                            let p = i + j * omr.b;
                            if kart.hojd.varden[p] < 0.0 {
                                continue;
                            }
                            if kart.vattentyp.varden[p] == Vatten::Flodfara {
                                n_fara += 1;
                            }
                        }
                        assert!(n_fara > 0, "levande fåra utan Flodfara fro={fro}");
                    }
                } else {
                    let a = and.as_ref().unwrap();
                    for path in &linjer {
                        let mut ok = false;
                        for &(i, j) in path {
                            let p = i + j * omr.b;
                            if kart.hojd.varden[p] < 0.0 {
                                continue;
                            }
                            if a.fara_andel.varden[p] > 0.0 {
                                ok = true;
                                break;
                            }
                        }
                        assert!(ok, "levande fåra utan fara_andel fro={fro}");
                    }
                    // Rutor som är Trask ska ha fara_andel 0.
                    for (p, &vt) in kart.vattentyp.varden.iter().enumerate() {
                        if vt == Vatten::Trask && !levande_mitt[p] {
                            assert!(
                                a.fara_andel.varden[p] == 0.0,
                                "Trask med fara_andel > 0"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Prov 7: ingen ändring utan händelse — mellan två t där nasta_andring pekar
/// förbi intervallet (inga period- eller loppbyten emellan) är kartorna lika.
#[test]
fn prov7_ingen_andring_utan_handelse() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fro = 181u64;
    // Hitta ett intervall utan händelse: gå bakåt från en känd tid.
    let t_hi = 200_000.0;
    let nasta = nasta_andring(&tt, t_hi, fro).expect("nasta");
    // Välj t_lo strax ovanför nasta (ingen händelse i (nasta, t_hi]).
    let t_lo = nasta + 1.0;
    assert!(t_lo < t_hi);
    for version in [1u8, 2] {
        let (a, and_a) = karta_for_ar(&k, &tt, &omr, t_hi, fro, version, 1).unwrap();
        let (b, and_b) = karta_for_ar(&k, &tt, &omr, t_lo, fro, version, 1).unwrap();
        assert_eq!(a.vattentyp.varden, b.vattentyp.varden);
        if version == 2 {
            assert_eq!(
                and_a.as_ref().unwrap().fara_andel.varden,
                and_b.as_ref().unwrap().fara_andel.varden
            );
        }
    }
}

/// Prov 8: händelserna nasta_andring och periodgränser.
/// Efter Alt 1395 pekar `nasta_andring(tt, t, fro)` på nästa period- *eller* loppbyte;
/// periodgränserna ensamma nås via `nasta_andring_period`.
#[test]
fn prov8_handelserna() {
    let tt = tidtabell();
    assert_eq!(hckarta::nasta_andring_period(&tt, 500_000.0), Some(290_000.0));
    assert_eq!(hckarta::nasta_andring_period(&tt, 20_000.0), Some(0.0));
    assert_eq!(hckarta::nasta_andring_period(&tt, 0.0), None);
    // Med fro: närmaste ändring kan vara ett loppbyte (inte bara 290000).
    let n = nasta_andring(&tt, 500_000.0, 181);
    assert!(n.is_some());
    assert!(n.unwrap() < 500_000.0);
    assert_eq!(nasta_andring(&tt, 0.0, 181), None);
    assert_eq!(tt.period(290_001.0).unwrap(), Period::Aldre);
    assert_eq!(tt.period(290_000.0).unwrap(), Period::Ovre);
}

/// Prov 9: hav oförändrat vid 500000, 20000 och 0.
#[test]
fn prov9_hav() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    // Hitta eventuella havsrutor på en bas-karta utan flod.
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let hav_ix: Vec<usize> = bas
        .hojd
        .varden
        .iter()
        .enumerate()
        .filter(|(_, &h)| h < 0.0)
        .map(|(i, _)| i)
        .collect();
    for version in [1u8, 2] {
        let maps: Vec<_> = [500_000.0, 20_000.0, 0.0]
            .iter()
            .map(|&t| karta_for_ar(&k, &tt, &omr, t, 181, version, 1).unwrap())
            .collect();
        for &p in &hav_ix {
            let v0 = maps[0].0.vattentyp.varden[p];
            let y0 = maps[0].0.vatten_yta.varden[p];
            for (kart, _) in &maps {
                assert_eq!(kart.vattentyp.varden[p], v0);
                assert_eq!(kart.vatten_yta.varden[p], y0);
                assert_eq!(kart.vattentyp.varden[p], Vatten::Hav);
            }
        }
    }
}

/// Prov 10: samma frö → samma karta; urvalet skiljer för minst två av fröna 1–50.
#[test]
fn prov10_samma_fro_och_urval() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (a1, _) = karta_for_ar(&k, &tt, &omr, 20_000.0, 42, 1, 1).unwrap();
    let (a2, _) = karta_for_ar(&k, &tt, &omr, 20_000.0, 42, 1, 1).unwrap();
    assert_eq!(kontrollsumma(&a1), kontrollsumma(&a2));

    let mut sedda = std::collections::BTreeSet::new();
    for fro in 1u64..=50 {
        sedda.insert(levande_foror_nr(fro, Period::Ovre));
    }
    assert!(
        sedda.len() >= 2,
        "urvalet ska skilja för minst två frön 1–50, fick {} unika",
        sedda.len()
    );
}

/// Prov 11: ankarvärdena efter Alt 1405.
#[test]
fn prov11_ankarvardena() {
    let g = flodregler(Period::Gransen);
    assert_eq!(g.foror, 2);
    assert_eq!(g.bredd_rutor, 2);
    assert_eq!(g.trask_avstand, 3);
    assert!((g.sjoar_per_100_rutor - 2.0).abs() < 1e-12);
    let o = flodregler(Period::Ovre);
    assert!(g.sjoar_per_100_rutor >= o.sjoar_per_100_rutor);
}

/// Prov 12: LASMIG märker nya tal/regler som [A] eller eget val (inte «uppmätt»).
#[test]
fn prov12_markningen_lasmig() {
    let text =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("LASMIG.md"))
            .expect("LASMIG.md");
    let lower = text.to_lowercase();
    assert!(
        !lower.contains("uppmätt"),
        "LASMIG får inte märka något som uppmätt"
    );
    assert!(
        text.contains("eget val") || text.contains("**eget val**"),
        "LASMIG ska ange egna val"
    );
    assert!(
        text.contains("[A]") || text.contains("Butler") || text.contains("Hasler"),
        "LASMIG ska peka på källor [A]"
    );
    // De fyra egna valen från underlaget ska synas.
    assert!(text.contains("LEVFAROR") || text.contains("0x4C45_5646_4152_4F52"));
    assert!(text.contains("övergiven") || text.contains("Övergiven"));
}
