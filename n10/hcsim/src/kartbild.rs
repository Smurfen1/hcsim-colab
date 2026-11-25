//! Kartbilden (Alt 1595, Håkan 2026-11-22: "Det måste gå att se vars på kartan djuren trängs"): simuleringen skriver
//! medan den kör en ögonblicksbild av kartan (växter, tak, vatten) och djurens rutor till en fil, som `kartvy` ritar.
//! Läser bara: världen, djuren och slumpföljderna ändras inte (prov: samma djur_summa med och utan bilder).

use std::path::Path;

use hckarta::{Kalla, Tidtabell, Vatten};

use crate::formaga::utforsk_armar;
use crate::sim::Sim;
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, STEG_PER_AR};
use crate::sim::KantLage;
use crate::utforsk::{bygg_sim_balte, B_FULL, H_FULL};

/// Bältets bredd runt fönstret i visningskörningen (Håkans Alt 1725; agenternas Alt 1718, 1719; strykprovet: 8 rutor med sammanfogade
/// sidor gav jämn fördelning i fönstret och i bältet). Eget val.
pub const BALTE_K: usize = 8;

/// Rättningarna mot utdöendet i visningskörningen (Håkans Alt 1825, 2026-11-23; agenternas omröstning Alt 1797, 1799, 1800, 1802, alla fyra
/// röstare eniga; protokollet `n10/docs/2026-11-23-omrostning-utdod.md`). Mätt på fröna 261–280 i 100 år: 0 av 20 världar dör ut, mot 20 av 20
/// år 21 utan rättningarna (`work/hcsim/utdod/e120-20fron.tsv`, `torus-k8-20fron.tsv`).
/// Könsmognad vid 3 år = 450 steg för båda könen: lånat från *Maiasaura* (Woodward m.fl. 2015, Paleobiology 41:503–527).
pub const UTDOD_MOGEN_STEG: i64 = 450;
/// Energitalen gånger 150/960, så att taket är 150 steg för båda könen: lånat från däggdjur (Lindstedt och Boyce 1985); `E_DELA` och
/// `E_START` behåller låsets kvoter (37,5 och 18,75).
pub const UTDOD_E_SKALA: f64 = 150.0 / 960.0;
/// Honan föder bara vid minst 80 % av taket (120 av 150). Eget val; ingen källa ger talet.
pub const UTDOD_FODSEL_TROSKEL: f64 = 120.0;

/// Slår på rättningarna i en nybyggd värld (år 0) och skalar startdjurens energi med samma tal. Växlarna sparas inte i ett sparat läge,
/// så funktionen anropas vid varje start. Intaget följer rutans fyllnad (högst 3,2 per steg som i låset v3.0); en födsel per hona och år
/// (steg 45, eget val).
pub fn satt_rattningar_utdod(sim: &mut Sim) {
    sim.intag_per_fyllnad = true;
    sim.hona_vuxen = true;
    sim.fodsel_arlig = true;
    sim.mogen_steg = Some(UTDOD_MOGEN_STEG);
    sim.e_skala = UTDOD_E_SKALA;
    sim.fodsel_troskel = Some(UTDOD_FODSEL_TROSKEL);
    for d in sim.djur.iter_mut() {
        d.e *= UTDOD_E_SKALA;
    }
}

/// Fönstret som visas: (k, b, h). Utan bälte hela kartan.
fn fonster(sim: &Sim) -> (usize, usize, usize) {
    let k = sim.fonster_k;
    (k, sim.b - 2 * k, sim.h - 2 * k)
}

/// Rutornas index i fönstret, rad för rad.
fn fonster_celler(sim: &Sim) -> Vec<usize> {
    let (k, b, h) = fonster(sim);
    let mut ut = Vec::with_capacity(b * h);
    for j in 0..h {
        for i in 0..b {
            ut.push((j + k) * sim.b + (i + k));
        }
    }
    ut
}

/// Årstid ur årets fas (0,0 = vintersolståndet, 0,5 = sommarsolståndet).
pub fn arstid_av_fas(fas: f64) -> &'static str {
    let f = fas.rem_euclid(1.0);
    if !(0.125..0.875).contains(&f) {
        "vinter"
    } else if f < 0.375 {
        "var"
    } else if f < 0.625 {
        "sommar"
    } else {
        "host"
    }
}

/// Dag på året (1..=365) ur årets fas.
pub fn dag_pa_aret_av_fas(fas: f64) -> i32 {
    let f = fas.rem_euclid(1.0);
    ((f * 365.0).floor() as i32 + 1).clamp(1, 365)
}

/// Levande djur i fönstret med fönstrets koordinater och bakåtkompatibla visningsfält.
struct FonsterDjur {
    i: usize,
    j: usize,
    markt: bool,
    art_id: usize,
    alder_ar: f64,
    hona: bool,
    energi: f64,
}

fn fonster_djur_full(sim: &Sim) -> Vec<FonsterDjur> {
    let (k, b, h) = fonster(sim);
    let steg_ar = STEG_PER_AR as f64;
    sim.djur
        .iter()
        .filter(|d| d.lev && d.i >= k && d.j >= k && d.i < k + b && d.j < k + h)
        .map(|d| FonsterDjur {
            i: d.i - k,
            j: d.j - k,
            markt: d.markt,
            art_id: 0, // Nuvarande hcsim simulerar enbart Edmontosaurus.
            alder_ar: d.age as f64 / steg_ar,
            hona: d.hona,
            energi: d.e,
        })
        .collect()
}

/// Levande djur i fönstret med fönstrets koordinater.
fn fonster_djur(sim: &Sim) -> Vec<(usize, usize, bool)> {
    fonster_djur_full(sim)
        .into_iter()
        .map(|d| (d.i, d.j, d.markt))
        .collect()
}

/// Ögonblicksbilden som JSON (en rad).
/// Bakåtkompatibel: `djur` behåller `[kolumn, rad, märkt (0/1)]`, medan extra fält
/// (`artnamn`, `djur_art`, `djur_alder_ar`, `djur_hona`, `djur_energi`, `arstid`, `ar_fas`,
/// `dag_pa_aret`, `temp_c`, `vat_sasong`, `landandel`) ligger parallellt på toppnivån.
pub fn bild_json(sim: &Sim, rubrik: &str) -> String {
    let celler = fonster_celler(sim);
    let (_, fb, fh) = fonster(sim);
    let v: Vec<String> = celler.iter().map(|&c| format!("{:.3}", sim.v[c])).collect();
    let tak: Vec<String> = celler.iter().map(|&c| format!("{:.3}", sim.vmax[c])).collect();
    let landandel: Vec<String> = celler.iter().map(|&c| format!("{:.3}", sim.landandel[c])).collect();
    let vatten: Vec<String> = celler.iter().map(|&c| (sim.vatten[c] as u8).to_string()).collect();
    let djur_lista = fonster_djur_full(sim);
    let djur: Vec<String> = djur_lista
        .iter()
        .map(|d| format!("[{},{},{}]", d.i, d.j, if d.markt { 1 } else { 0 }))
        .collect();
    let djur_art: Vec<String> = djur_lista.iter().map(|d| d.art_id.to_string()).collect();
    let djur_alder_ar: Vec<String> = djur_lista.iter().map(|d| format!("{:.2}", d.alder_ar)).collect();
    let djur_hona: Vec<String> = djur_lista.iter().map(|d| if d.hona { "1" } else { "0" }.to_string()).collect();
    let djur_energi: Vec<String> = djur_lista.iter().map(|d| format!("{:.1}", d.energi)).collect();
    let ar_fas = sim.klimat.fas(sim.t).rem_euclid(1.0);
    let arstid = arstid_av_fas(ar_fas);
    let dag_pa_aret = dag_pa_aret_av_fas(ar_fas);
    let temp_c = sim.klimat.temp(ar_fas);
    let vat_sasong = sim.klimat.vat(ar_fas);
    let extra_vaxt = if sim.kron_tathet.len() == sim.v.len() && sim.vaxt_grupper.len() == sim.v.len() {
        let kt: Vec<String> = celler.iter().map(|&c| format!("{:.3}", sim.kron_tathet[c])).collect();
        let vg: Vec<String> = celler
            .iter()
            .map(|&c| {
                let g = sim.vaxt_grupper[c];
                format!(
                    "[{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}]",
                    g[0], g[1], g[2], g[3], g[4], g[5]
                )
            })
            .collect();
        format!(
            ",\"kron_tathet\":[{}],\"vaxt_grupper_namn\":[\"lovtrad\",\"barrtrad\",\"ort\",\"ormbunke\",\"kottepalm_ginkgo\",\"vattenvaxt\"],\"vaxt_grupper\":[{}]",
            kt.join(","),
            vg.join(",")
        )
    } else {
        String::new()
    };
    format!(
        "{{\"rubrik\":{},\"t\":{},\"ar\":{:.2},\"arstid\":\"{}\",\"ar_fas\":{:.4},\"dag_pa_aret\":{},\"temp_c\":{:.1},\"vat_sasong\":{},\"markerad\":{},\"b\":{},\"h\":{},\"artnamn\":[\"Edmontosaurus\"],\"v\":[{}],\"vmax\":[{}],\"landandel\":[{}],\"vatten\":[{}],\"djur\":[{}],\"djur_art\":[{}],\"djur_alder_ar\":[{}],\"djur_hona\":[{}],\"djur_energi\":[{}]{}}}",
        serde_json::to_string(rubrik).unwrap_or_else(|_| "\"\"".into()),
        sim.t,
        sim.t as f64 / STEG_PER_AR as f64,
        arstid,
        ar_fas,
        dag_pa_aret,
        temp_c,
        vat_sasong,
        sim.markering_skedd,
        fb,
        fh,
        v.join(","),
        tak.join(","),
        landandel.join(","),
        vatten.join(","),
        djur.join(","),
        djur_art.join(","),
        djur_alder_ar.join(","),
        djur_hona.join(","),
        djur_energi.join(","),
        extra_vaxt
    )
}

/// Skriver bilden så att läsaren aldrig ser en halv fil (först till .tmp, sedan byte av namn).
pub fn skriv_bild(sim: &Sim, rubrik: &str, ut: &Path) -> Result<(), String> {
    let tmp = ut.with_extension("tmp");
    std::fs::write(&tmp, bild_json(sim, rubrik)).map_err(|e| format!("skriv {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, ut).map_err(|e| format!("byt namn till {}: {e}", ut.display()))
}

/// Tidsskiftets fil bredvid kartbilden: `FIL.historik.jsonl`, en bild per rad med fältet `klocka` (sekunder sedan 1970).
pub fn historik_vag(ut: &Path) -> std::path::PathBuf {
    let mut s = ut.as_os_str().to_owned();
    s.push(".historik.jsonl");
    std::path::PathBuf::from(s)
}

/// En rad till tidsskiftet: bilden med klockslaget först.
pub fn historik_rad(sim: &Sim, rubrik: &str, klocka: u64) -> String {
    let j = bild_json(sim, rubrik);
    format!("{{\"klocka\":{klocka},{}\n", &j[1..])
}

/// Världens fasta värden till tidsskiftets databas.
pub fn fast_ur_sim(sim: &Sim, rubrik: &str) -> crate::tidsskift::Fast {
    crate::tidsskift::Fast {
        rubrik: rubrik.to_string(),
        b: fonster(sim).1,
        h: fonster(sim).2,
        steg_per_ar: STEG_PER_AR,
        vmax: fonster_celler(sim).iter().map(|&c| sim.vmax[c] as f32).collect(),
        vatten: fonster_celler(sim).iter().map(|&c| sim.vatten[c] as u8).collect(),
    }
}

/// Världens läge just nu till tidsskiftets databas: bara värdena som ändras.
pub fn lage_ur_sim(sim: &Sim, klocka: u64) -> crate::tidsskift::Lage {
    crate::tidsskift::Lage {
        klocka,
        t: sim.t,
        markerad: sim.markering_skedd,
        v: fonster_celler(sim).iter().map(|&c| sim.v[c] as f32).collect(),
        djur: fonster_djur(sim).iter().map(|&(i, j, m)| (i as u8, j as u8, m)).collect(),
    }
}

fn klocka_nu() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[allow(dead_code)]
fn lagg_till_historik(sim: &Sim, rubrik: &str, vag: &Path) -> Result<(), String> {
    use std::io::Write;
    let klocka = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(vag).map_err(|e| format!("öppna {}: {e}", vag.display()))?;
    f.write_all(historik_rad(sim, rubrik, klocka).as_bytes()).map_err(|e| format!("skriv {}: {e}", vag.display()))
}

/// `hcsim kartbild --arm e_lar [--spegel] --fro 261 [--var-steg 30] [--historik-s 60] --ut FIL`: kör en värld ur Alt 1463:s
/// armar (samma byggnad, inkörning och märkning som försöket), skriver kartbilden var `var_steg` steg och sparar en bild i
/// tidsskiftet var `historik_s` sekund (standard 10, så att uppspelningen varje minut får bilder nog; Håkan 2026-11-22), och vid
/// start och slut. `slut_ar` (valet `--ar N`) byter slutåret; utan det gäller försökets längd.
pub fn kor_kartbild(rot: &Path, arm_namn: &str, spegel: bool, fro: u64, var_steg: i64, historik_s: u64, ut: &Path, slut_ar: Option<i64>) -> Result<(), String> {
    let arm = utforsk_armar()
        .into_iter()
        .find(|a| a.namn == arm_namn && a.spegel == spegel)
        .ok_or(format!("armen {arm_namn} (spegel {spegel}) finns inte bland Alt 1463:s armar"))?;
    let kalla = Kalla::las(&rot.join("../paleokarta/data")).map_err(|e| format!("karta: {e}"))?;
    let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text).map_err(|e| format!("tidtabell: {e}"))?;
    // Visningskörningen: bälte på BALTE_K rutor runt fönstret 64 · 48 och sammanfogade sidor (Håkans Alt 1725), så att djuren inte
    // samlas mot en kant. Kartbilden och tidsskiftet visar bara fönstret.
    let mut sim = bygg_sim_balte(&kalla, &tt, B_FULL, H_FULL, BALTE_K, fro, &arm)?;
    sim.kant = KantLage::Torus;
    sim.fonster_k = BALTE_K;
    satt_rattningar_utdod(&mut sim);
    let rubrik = format!(
        "armen {} (märkta {}, omärkta {}), frö {}, bälte {} rutor, sammanfogade sidor",
        arm.namn,
        arm.markta.som_str(),
        arm.omarkta.som_str(),
        fro,
        BALTE_K
    );
    let inkor = MARKT_INKOR_AR * STEG_PER_AR;
    // `--ar N` (Håkans Alt 1703, 2026-11-22: kartvyn live): kör till år N i stället för försökets 180 år.
    let slut = slut_ar.unwrap_or(MARKT_INKOR_AR + MARKT_EFTER_AR) * STEG_PER_AR;
    skriv_bild(&sim, &rubrik, ut)?;
    // Tidsskiftet sparas som värden i en databas (Håkan 2026-11-22), inte som en textrad per bild. När databasen når
    // sitt tak glesas den ut, och tiden mellan lägena dubblas.
    let _ = std::fs::remove_file(historik_vag(ut));
    let mut db = crate::tidsskift::Skrivare::ny(&crate::tidsskift::db_vag(ut), &fast_ur_sim(&sim, &rubrik))?;
    db.lagg_till(&lage_ur_sim(&sim, klocka_nu()))?;
    let mut historik_s = historik_s.max(1);
    let mut senast_hist = std::time::Instant::now();
    while sim.t < slut {
        if sim.t == inkor && !sim.markering_skedd {
            sim.markera_halften(fro);
        }
        sim.steg();
        if sim.t % var_steg.max(1) == 0 {
            skriv_bild(&sim, &rubrik, ut)?;
        }
        // Håkan 2026-11-22: "Det är ingen idé att köra 10 000 år om djuren har dött ut för länge sedan." Körningen slutar när
        // det sista djuret är dött; sista bilden och sista läget skrivs efter slingan.
        if sim.antal() == 0 {
            eprintln!("alla djur är döda vid år {:.2}; körningen avslutas", sim.t as f64 / STEG_PER_AR as f64);
            break;
        }
        if senast_hist.elapsed().as_secs() >= historik_s {
            if db.lagg_till(&lage_ur_sim(&sim, klocka_nu()))? {
                historik_s *= 2;
            }
            senast_hist = std::time::Instant::now();
        }
    }
    skriv_bild(&sim, &rubrik, ut)?;
    db.lagg_till(&lage_ur_sim(&sim, klocka_nu()))?;
    eprintln!("kartbilden klar: {} (år {})", ut.display(), sim.t / STEG_PER_AR);
    Ok(())
}

/// Vatten som djuren inte kan stå i ritas som vatten i kartvyn (land = 0).
pub fn ar_vatten(kod: u8) -> bool {
    kod != Vatten::Land as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use serde::Deserialize;

    /// Speglar äldre `HcKartData` i `n10/dinobiom/src/sim/hckartdata.rs` för att
    /// garantera 100 % bakåtkompatibilitet med live-3D-vyn.
    #[derive(Deserialize)]
    struct GammalKartData {
        #[serde(default)]
        rubrik: String,
        t: i64,
        ar: f64,
        #[serde(default)]
        markerad: bool,
        b: usize,
        h: usize,
        v: Vec<f32>,
        vmax: Vec<f32>,
        vatten: Vec<u8>,
        djur: Vec<[i32; 3]>,
    }

    #[test]
    fn arstid_och_dag_fran_fas() {
        assert_eq!(arstid_av_fas(0.0), "vinter");
        assert_eq!(arstid_av_fas(0.124), "vinter");
        assert_eq!(arstid_av_fas(0.125), "var");
        assert_eq!(arstid_av_fas(0.374), "var");
        assert_eq!(arstid_av_fas(0.375), "sommar");
        assert_eq!(arstid_av_fas(0.624), "sommar");
        assert_eq!(arstid_av_fas(0.625), "host");
        assert_eq!(arstid_av_fas(0.874), "host");
        assert_eq!(arstid_av_fas(0.875), "vinter");
        assert_eq!(dag_pa_aret_av_fas(0.0), 1);
        assert_eq!(dag_pa_aret_av_fas(0.5), 183);
        assert_eq!(dag_pa_aret_av_fas(0.9999), 365);
    }

    #[test]
    fn bild_json_bakatkompatibel_och_nya_artfalt() {
        let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let k = Kalla::las(&rot.join("../paleokarta/data")).unwrap();
        let tt = Tidtabell::las(&std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).unwrap()).unwrap();
        let arm = utforsk_armar().into_iter().find(|x| x.namn == "e_lar" && !x.spegel).unwrap();
        let mut sim = bygg_sim_balte(&k, &tt, 64, 48, 8, 261, &arm).unwrap();
        sim.fonster_k = 8;
        sim.t = 75; // halvvägs genom året -> sommar

        let text = bild_json(&sim, "prov-artfalt");

        // 1. Gammal 3D-läsare (med Vec<[i32; 3]>) måste kunna läsa filen oförändrad.
        let gammal: GammalKartData = serde_json::from_str(&text).expect("bakåtkompatibilitet bruten");
        assert_eq!(gammal.rubrik, "prov-artfalt");
        assert_eq!(gammal.t, 75);
        assert!((gammal.ar - 0.5).abs() < 1e-6);
        assert!(!gammal.markerad);
        assert_eq!((gammal.b, gammal.h), (64, 48));
        assert_eq!(gammal.v.len(), 64 * 48);
        assert_eq!(gammal.vmax.len(), 64 * 48);
        assert_eq!(gammal.vatten.len(), 64 * 48);
        assert!(!gammal.djur.is_empty());

        // 2. Nya parallella art-, ålders-, köns-, energi-, landandel- och årstidsfält.
        let j: serde_json::Value = serde_json::from_str(&text).unwrap();
        let n_djur = gammal.djur.len();
        assert_eq!(j["artnamn"].as_array().unwrap(), &vec![serde_json::json!("Edmontosaurus")]);
        assert_eq!(j["djur_art"].as_array().unwrap().len(), n_djur);
        assert_eq!(j["djur_alder_ar"].as_array().unwrap().len(), n_djur);
        assert_eq!(j["djur_hona"].as_array().unwrap().len(), n_djur);
        assert_eq!(j["djur_energi"].as_array().unwrap().len(), n_djur);
        assert_eq!(j["landandel"].as_array().unwrap().len(), 64 * 48);
        assert_eq!(j["arstid"].as_str(), Some("sommar"));
        assert_eq!(j["dag_pa_aret"].as_i64(), Some(183));
        assert!(j["temp_c"].as_f64().unwrap().is_finite());
        assert!(j["vat_sasong"].is_boolean());
        // Alla art-ID ska peka inom artnamn.
        for v in j["djur_art"].as_array().unwrap() {
            assert_eq!(v.as_u64(), Some(0));
        }
    }
}
