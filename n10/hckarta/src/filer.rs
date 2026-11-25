//! Filerna till 3D-bilden: JSON-rubrik, binära rutnät, 16-bitars PNG och strandlinje.
//! Inga externa paket; PNG med okomprimerade deflate-block, CRC-32 och Adler-32.

use std::fs;
use std::path::Path;

use crate::{
    bygg_vaxtgrupper_json, kontrollsumma_med, parsa_vaxtgrupper_json, FlodAndelar, ForaInfo, Karta,
    Lager, Marke, Omrade, Period, Vatten, VAXT_N,
};

/// Skriver mappen: karta.json (rubriken), hojd.f32, vatten_yta.f32, vattentyp.u8, mark.f32 (4 tal per ruta),
/// vaxt.f32, period.u8, hojd.png (16 bitar gråskala, värdet = (höjd + 1000 m) · 10, avrundat och begränsat
/// till 0..65535), strand.json (strandlinjen).
/// Alla binära rutnät little endian, radvis från sydväst (rad j = 0 är södra kanten, ruta i = 0 västra), b · h värden.
/// Returnerar kontrollsumman, som också står i karta.json. Version 1 (inga andelar): samma filer som före Alt 1343.
pub fn skriv_3d(k: &Karta, mapp: &Path) -> Result<u64, String> {
    skriv_3d_med(k, None, mapp)
}

/// Som `skriv_3d`; med `Some(andelar)` skrivs även `fara_andel.f32`, `balte_andel.f32`,
/// `vall_andel.f32` / `vall_hojd.f32` (om `har_vall`), `vaxt_tathet.f32` / `vaxtgrupper.json`
/// (om `har_vaxtgrupper`), `foror.tsv` (om `foror` inte är tom) och `"flodversion": 2`.
pub fn skriv_3d_med(k: &Karta, andelar: Option<&FlodAndelar>, mapp: &Path) -> Result<u64, String> {
    fs::create_dir_all(mapp).map_err(|e| format!("kunde inte skapa mapp {}: {e}", mapp.display()))?;
    let n = k.omrade.b * k.omrade.h;
    if k.hojd.varden.len() != n
        || k.vatten_yta.varden.len() != n
        || k.vattentyp.varden.len() != n
        || k.mark.varden.len() != n
        || k.vaxt.varden.len() != n
        || k.period.varden.len() != n
    {
        return Err("lagrens längd stämmer inte med området".into());
    }
    if let Some(a) = andelar {
        if a.fara_andel.varden.len() != n || a.balte_andel.varden.len() != n {
            return Err("andelslagrens längd stämmer inte med området".into());
        }
        if a.har_vall
            && (a.vall_andel.varden.len() != n || a.vall_hojd_m.varden.len() != n)
        {
            return Err("vallagrenas längd stämmer inte med området".into());
        }
        if a.har_vaxtgrupper && a.vaxt_tathet.varden.len() != n {
            return Err("växttäthetens längd stämmer inte med området".into());
        }
    }

    skriv_f32(mapp.join("hojd.f32"), &k.hojd.varden)?;
    skriv_f32(mapp.join("vatten_yta.f32"), &k.vatten_yta.varden)?;
    skriv_u8(
        mapp.join("vattentyp.u8"),
        &k.vattentyp.varden.iter().map(|v| *v as u8).collect::<Vec<_>>(),
    )?;
    skriv_f32x4(mapp.join("mark.f32"), &k.mark.varden)?;
    skriv_f32(mapp.join("vaxt.f32"), &k.vaxt.varden)?;
    skriv_u8(
        mapp.join("period.u8"),
        &k.period.varden.iter().map(|v| *v as u8).collect::<Vec<_>>(),
    )?;
    if let Some(a) = andelar {
        skriv_f32(mapp.join("fara_andel.f32"), &a.fara_andel.varden)?;
        skriv_f32(mapp.join("balte_andel.f32"), &a.balte_andel.varden)?;
        if a.har_vall {
            skriv_f32(mapp.join("vall_andel.f32"), &a.vall_andel.varden)?;
            skriv_f32(mapp.join("vall_hojd.f32"), &a.vall_hojd_m.varden)?;
        }
        if a.har_vaxtgrupper {
            skriv_f32x6(mapp.join("vaxt_tathet.f32"), &a.vaxt_tathet.varden)?;
            let json = bygg_vaxtgrupper_json(&a.vaxt_arter);
            fs::write(mapp.join("vaxtgrupper.json"), json.as_bytes())
                .map_err(|e| format!("vaxtgrupper.json: {e}"))?;
        }
        if !a.foror.is_empty() {
            skriv_foror_tsv(mapp.join("foror.tsv"), &a.foror, a.har_vall)?;
        }
    }

    let png = bygg_hojd_png(k.omrade.b, k.omrade.h, &k.hojd.varden)?;
    fs::write(mapp.join("hojd.png"), &png).map_err(|e| format!("hojd.png: {e}"))?;

    let kurvor = strandlinje(k);
    skriv_strand_json(mapp.join("strand.json"), &kurvor, k.omrade.ruta_m)?;

    let summa = kontrollsumma_med(k, andelar);
    let json = bygg_karta_json(k, summa, andelar);
    fs::write(mapp.join("karta.json"), json.as_bytes()).map_err(|e| format!("karta.json: {e}"))?;
    Ok(summa)
}

/// Läser mappen tillbaka till en Karta (allt utom PNG:n och strandlinjen, som bara är till 3D).
/// Fel om en fil saknas, har fel längd, eller om kontrollsumman för den lästa kartan skiljer sig från den i karta.json.
/// Läser både flodversion 1 och 2 (andelarna verifieras i summan men returneras inte här).
pub fn las_3d(mapp: &Path) -> Result<Karta, String> {
    Ok(las_3d_med(mapp)?.0)
}

/// Som `las_3d`, och returnerar andelslagren om mappen är flodversion 2.
pub fn las_3d_med(mapp: &Path) -> Result<(Karta, Option<FlodAndelar>), String> {
    let text = fs::read_to_string(mapp.join("karta.json"))
        .map_err(|e| format!("karta.json saknas eller kan inte läsas: {e}"))?;
    let rubrik = parsa_karta_json(&text)?;

    let n = rubrik.omrade.b * rubrik.omrade.h;
    let hojd = las_f32(mapp.join("hojd.f32"), n)?;
    let vatten_yta = las_f32(mapp.join("vatten_yta.f32"), n)?;
    let vattentyp_u8 = las_u8(mapp.join("vattentyp.u8"), n)?;
    let mark = las_f32x4(mapp.join("mark.f32"), n)?;
    let vaxt = las_f32(mapp.join("vaxt.f32"), n)?;
    let period_u8 = las_u8(mapp.join("period.u8"), n)?;

    let vattentyp = vattentyp_u8
        .into_iter()
        .map(vatten_fran_u8)
        .collect::<Result<Vec<_>, _>>()?;
    let period = period_u8
        .into_iter()
        .map(period_fran_u8)
        .collect::<Result<Vec<_>, _>>()?;

    let andelar = if rubrik.flodversion >= 2 {
        let fara = las_f32(mapp.join("fara_andel.f32"), n)?;
        let balte = las_f32(mapp.join("balte_andel.f32"), n)?;
        let marke_fara = rubrik
            .marke_fara_andel
            .ok_or_else(|| "flodversion 2 saknar märke för fara_andel".to_string())?;
        let marke_balte = rubrik
            .marke_balte_andel
            .ok_or_else(|| "flodversion 2 saknar märke för balte_andel".to_string())?;
        // Alt 1343 saknar foror.tsv — tom lista, samma kontrollsumma som före Alt 1386.
        let foror = if mapp.join("foror.tsv").exists() {
            las_foror_tsv(mapp.join("foror.tsv"))?
        } else {
            Vec::new()
        };
        // Alt 1343/1386 saknar vallfiler — nollager, samma kontrollsumma som före Alt 1371.
        let har_vall = mapp.join("vall_andel.f32").exists();
        let (vall, vall_hojd, marke_vall, marke_vall_hojd) = if har_vall {
            let marke_vall = rubrik
                .marke_vall_andel
                .ok_or_else(|| "flodversion 2 med vall saknar märke för vall_andel".to_string())?;
            let marke_vall_hojd = rubrik
                .marke_vall_hojd
                .ok_or_else(|| "flodversion 2 med vall saknar märke för vall_hojd".to_string())?;
            (
                las_f32(mapp.join("vall_andel.f32"), n)?,
                las_f32(mapp.join("vall_hojd.f32"), n)?,
                marke_vall,
                marke_vall_hojd,
            )
        } else {
            (
                vec![0.0f32; n],
                vec![0.0f32; n],
                Marke::Simulering,
                Marke::Simulering,
            )
        };
        // Alt 1343/1386/1371 saknar växtgrupper — nollager, samma kontrollsumma som före Alt 1433.
        let har_vaxtgrupper = mapp.join("vaxt_tathet.f32").exists();
        let (vaxt_tathet, marke_vaxt_tathet, vaxt_hojd, vaxt_arter) = if har_vaxtgrupper {
            let marke_vaxt_tathet = rubrik.marke_vaxt_tathet.ok_or_else(|| {
                "flodversion 2 med växtgrupper saknar märke för vaxt_tathet".to_string()
            })?;
            let json_text = fs::read_to_string(mapp.join("vaxtgrupper.json"))
                .map_err(|e| format!("vaxtgrupper.json saknas eller kan inte läsas: {e}"))?;
            let (arter, hojder) = parsa_vaxtgrupper_json(&json_text)?;
            (
                las_f32x6(mapp.join("vaxt_tathet.f32"), n)?,
                marke_vaxt_tathet,
                hojder,
                arter,
            )
        } else {
            (
                vec![[0.0f32; VAXT_N]; n],
                Marke::Simulering,
                [0.0f32; VAXT_N],
                [0usize; VAXT_N],
            )
        };
        Some(FlodAndelar {
            fara_andel: Lager {
                varden: fara,
                marke: marke_fara,
            },
            balte_andel: Lager {
                varden: balte,
                marke: marke_balte,
            },
            foror,
            har_vall,
            vall_andel: Lager {
                varden: vall,
                marke: marke_vall,
            },
            vall_hojd_m: Lager {
                varden: vall_hojd,
                marke: marke_vall_hojd,
            },
            har_vaxtgrupper,
            vaxt_tathet: Lager {
                varden: vaxt_tathet,
                marke: marke_vaxt_tathet,
            },
            vaxt_hojd,
            vaxt_arter,
        })
    } else {
        None
    };

    let k = Karta {
        omrade: rubrik.omrade,
        t: rubrik.t,
        hojd: Lager {
            varden: hojd,
            marke: rubrik.marke_hojd,
        },
        vatten_yta: Lager {
            varden: vatten_yta,
            marke: rubrik.marke_vatten_yta,
        },
        vattentyp: Lager {
            varden: vattentyp,
            marke: rubrik.marke_vattentyp,
        },
        mark: Lager {
            varden: mark,
            marke: rubrik.marke_mark,
        },
        vaxt: Lager {
            varden: vaxt,
            marke: rubrik.marke_vaxt,
        },
        period: Lager {
            varden: period,
            marke: rubrik.marke_period,
        },
    };

    let beraknad = kontrollsumma_med(&k, andelar.as_ref());
    if beraknad != rubrik.kontrollsumma {
        return Err(format!(
            "kontrollsumman stämmer inte: filen har {}, läst karta ger {}",
            rubrik.kontrollsumma, beraknad
        ));
    }
    Ok((k, andelar))
}

/// Strandlinjen: kurvorna där höjden går genom 0 m (marching squares över rutornas mittpunkter),
/// i rutornas koordinater (x = i + 0,5, y = j + 0,5). Tom lista om hela kartan är land eller hela är hav.
pub fn strandlinje(k: &Karta) -> Vec<Vec<(f64, f64)>> {
    let b = k.omrade.b;
    let h = k.omrade.h;
    if b < 2 || h < 2 {
        return Vec::new();
    }
    let hojd = &k.hojd.varden;
    let mut segment: Vec<[(f64, f64); 2]> = Vec::new();

    for j in 0..(h - 1) {
        for i in 0..(b - 1) {
            let v00 = hojd[i + j * b];
            let v10 = hojd[(i + 1) + j * b];
            let v11 = hojd[(i + 1) + (j + 1) * b];
            let v01 = hojd[i + (j + 1) * b];

            let p00 = (i as f64 + 0.5, j as f64 + 0.5);
            let p10 = ((i + 1) as f64 + 0.5, j as f64 + 0.5);
            let p11 = ((i + 1) as f64 + 0.5, (j + 1) as f64 + 0.5);
            let p01 = (i as f64 + 0.5, (j + 1) as f64 + 0.5);

            // Bit = 1 om höjden ≥ 0 (land/vattenyta), 0 om under havsytan.
            let mut kod = 0u8;
            if v00 >= 0.0 {
                kod |= 1;
            }
            if v10 >= 0.0 {
                kod |= 2;
            }
            if v11 >= 0.0 {
                kod |= 4;
            }
            if v01 >= 0.0 {
                kod |= 8;
            }

            let kant = |a: (f64, f64), va: f32, b: (f64, f64), vb: f32| -> (f64, f64) {
                interpolera_noll(a, va, b, vb)
            };

            // Kanter: botten (0–1), höger (1–2), topp (2–3), vänster (3–0).
            let botten = || kant(p00, v00, p10, v10);
            let hoger = || kant(p10, v10, p11, v11);
            let topp = || kant(p11, v11, p01, v01);
            let vanster = || kant(p01, v01, p00, v00);

            match kod {
                0 | 15 => {}
                1 | 14 => segment.push([vanster(), botten()]),
                2 | 13 => segment.push([botten(), hoger()]),
                3 | 12 => segment.push([vanster(), hoger()]),
                4 | 11 => segment.push([hoger(), topp()]),
                5 => {
                    // Sadel: välj botten–höger och vänster–topp.
                    segment.push([botten(), hoger()]);
                    segment.push([vanster(), topp()]);
                }
                6 | 9 => segment.push([botten(), topp()]),
                7 | 8 => segment.push([vanster(), topp()]),
                10 => {
                    segment.push([botten(), vanster()]);
                    segment.push([hoger(), topp()]);
                }
                _ => {}
            }
        }
    }

    sammanfoga_segment(segment)
}

// --- binära rutnät -----------------------------------------------------------

fn skriv_f32(path: impl AsRef<Path>, v: &[f32]) -> Result<(), String> {
    let mut buf = Vec::with_capacity(v.len() * 4);
    for x in v {
        buf.extend_from_slice(&x.to_le_bytes());
    }
    fs::write(path.as_ref(), buf).map_err(|e| format!("{}: {e}", path.as_ref().display()))
}

fn skriv_f32x4(path: impl AsRef<Path>, v: &[[f32; 4]]) -> Result<(), String> {
    let mut buf = Vec::with_capacity(v.len() * 16);
    for rad in v {
        for x in rad {
            buf.extend_from_slice(&x.to_le_bytes());
        }
    }
    fs::write(path.as_ref(), buf).map_err(|e| format!("{}: {e}", path.as_ref().display()))
}

fn skriv_f32x6(path: impl AsRef<Path>, v: &[[f32; VAXT_N]]) -> Result<(), String> {
    let mut buf = Vec::with_capacity(v.len() * VAXT_N * 4);
    for rad in v {
        for x in rad {
            buf.extend_from_slice(&x.to_le_bytes());
        }
    }
    fs::write(path.as_ref(), buf).map_err(|e| format!("{}: {e}", path.as_ref().display()))
}

fn skriv_u8(path: impl AsRef<Path>, v: &[u8]) -> Result<(), String> {
    fs::write(path.as_ref(), v).map_err(|e| format!("{}: {e}", path.as_ref().display()))
}

fn skriv_foror_tsv(path: impl AsRef<Path>, foror: &[ForaInfo], med_vall: bool) -> Result<(), String> {
    let mut s = if med_vall {
        String::from("fara\tbredd_m\tbalte_m\trutor\tvall_m\n")
    } else {
        String::from("fara\tbredd_m\tbalte_m\trutor\n")
    };
    for (i, f) in foror.iter().enumerate() {
        if med_vall {
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                i,
                fmt_f64(f.bredd_m),
                fmt_f64(f.balte_m),
                f.rutor,
                fmt_f64(f.vall_m)
            ));
        } else {
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                i,
                fmt_f64(f.bredd_m),
                fmt_f64(f.balte_m),
                f.rutor
            ));
        }
    }
    fs::write(path.as_ref(), s).map_err(|e| format!("{}: {e}", path.as_ref().display()))
}

fn las_foror_tsv(path: impl AsRef<Path>) -> Result<Vec<ForaInfo>, String> {
    let text = fs::read_to_string(path.as_ref())
        .map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    let mut foror = Vec::new();
    for (rad_nr, rad) in text.lines().enumerate() {
        let rad = rad.trim();
        if rad.is_empty() {
            continue;
        }
        if rad_nr == 0 && rad.starts_with("fara") {
            continue;
        }
        let delar: Vec<&str> = rad.split('\t').collect();
        if delar.len() != 4 && delar.len() != 5 {
            return Err(format!(
                "{}: rad {} har {} fält (väntade 4 eller 5)",
                path.as_ref().display(),
                rad_nr + 1,
                delar.len()
            ));
        }
        let bredd_m: f64 = delar[1]
            .parse()
            .map_err(|_| format!("{}: bredd_m {:?}", path.as_ref().display(), delar[1]))?;
        let balte_m: f64 = delar[2]
            .parse()
            .map_err(|_| format!("{}: balte_m {:?}", path.as_ref().display(), delar[2]))?;
        let rutor: usize = delar[3]
            .parse()
            .map_err(|_| format!("{}: rutor {:?}", path.as_ref().display(), delar[3]))?;
        let vall_m: f64 = if delar.len() == 5 {
            delar[4]
                .parse()
                .map_err(|_| format!("{}: vall_m {:?}", path.as_ref().display(), delar[4]))?
        } else {
            0.0
        };
        foror.push(ForaInfo {
            bredd_m,
            balte_m,
            rutor,
            vall_m,
        });
    }
    if foror.is_empty() {
        return Err(format!("{}: tom fil", path.as_ref().display()));
    }
    Ok(foror)
}

fn las_f32(path: impl AsRef<Path>, n: usize) -> Result<Vec<f32>, String> {
    let bytes = fs::read(path.as_ref()).map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    let vantat = n * 4;
    if bytes.len() != vantat {
        return Err(format!(
            "{}: fel längd {}, väntade {}",
            path.as_ref().display(),
            bytes.len(),
            vantat
        ));
    }
    let mut ut = Vec::with_capacity(n);
    for chunk in bytes.chunks_exact(4) {
        ut.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(ut)
}

fn las_f32x4(path: impl AsRef<Path>, n: usize) -> Result<Vec<[f32; 4]>, String> {
    let bytes = fs::read(path.as_ref()).map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    let vantat = n * 16;
    if bytes.len() != vantat {
        return Err(format!(
            "{}: fel längd {}, väntade {}",
            path.as_ref().display(),
            bytes.len(),
            vantat
        ));
    }
    let mut ut = Vec::with_capacity(n);
    for chunk in bytes.chunks_exact(16) {
        let mut rad = [0f32; 4];
        for i in 0..4 {
            let o = i * 4;
            rad[i] = f32::from_le_bytes([chunk[o], chunk[o + 1], chunk[o + 2], chunk[o + 3]]);
        }
        ut.push(rad);
    }
    Ok(ut)
}

fn las_f32x6(path: impl AsRef<Path>, n: usize) -> Result<Vec<[f32; VAXT_N]>, String> {
    let bytes = fs::read(path.as_ref()).map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    let vantat = n * VAXT_N * 4;
    if bytes.len() != vantat {
        return Err(format!(
            "{}: fel längd {}, väntade {}",
            path.as_ref().display(),
            bytes.len(),
            vantat
        ));
    }
    let mut ut = Vec::with_capacity(n);
    for chunk in bytes.chunks_exact(VAXT_N * 4) {
        let mut rad = [0f32; VAXT_N];
        for i in 0..VAXT_N {
            let o = i * 4;
            rad[i] = f32::from_le_bytes([chunk[o], chunk[o + 1], chunk[o + 2], chunk[o + 3]]);
        }
        ut.push(rad);
    }
    Ok(ut)
}

fn las_u8(path: impl AsRef<Path>, n: usize) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path.as_ref()).map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    if bytes.len() != n {
        return Err(format!(
            "{}: fel längd {}, väntade {}",
            path.as_ref().display(),
            bytes.len(),
            n
        ));
    }
    Ok(bytes)
}

fn vatten_fran_u8(v: u8) -> Result<Vatten, String> {
    match v {
        0 => Ok(Vatten::Land),
        1 => Ok(Vatten::Flodfara),
        2 => Ok(Vatten::Trask),
        3 => Ok(Vatten::Sjo),
        4 => Ok(Vatten::Brackt),
        5 => Ok(Vatten::Hav),
        _ => Err(format!("okänd vattentyp {v}")),
    }
}

fn period_fran_u8(v: u8) -> Result<Period, String> {
    match v {
        0 => Ok(Period::Aldre),
        1 => Ok(Period::Ovre),
        2 => Ok(Period::Gransen),
        _ => Err(format!("okänd period {v}")),
    }
}

fn marke_str(m: Marke) -> &'static str {
    match m {
        Marke::Last => "läst",
        Marke::Simulering => "simulering",
    }
}

fn marke_fran_str(s: &str) -> Result<Marke, String> {
    match s {
        "läst" => Ok(Marke::Last),
        "simulering" => Ok(Marke::Simulering),
        _ => Err(format!("okänt märke {s:?}")),
    }
}

fn period_namn(p: Period) -> &'static str {
    match p {
        Period::Aldre => "Aldre",
        Period::Ovre => "Ovre",
        Period::Gransen => "Gransen",
    }
}

// --- JSON --------------------------------------------------------------------

fn fmt_f64(x: f64) -> String {
    // Tillräckligt många siffror för att läsa tillbaka samma omrade/t.
    let s = format!("{:.17}", x);
    // Trimma onödiga nollor men behåll minst en decimal.
    if let Some(dot) = s.find('.') {
        let (hel, rest) = s.split_at(dot);
        let rest = rest.trim_end_matches('0');
        if rest == "." {
            format!("{hel}.0")
        } else {
            format!("{hel}{rest}")
        }
    } else {
        s
    }
}

fn bygg_karta_json(k: &Karta, summa: u64, andelar: Option<&FlodAndelar>) -> String {
    let o = &k.omrade;
    let period = k
        .period
        .varden
        .first()
        .copied()
        .map(period_namn)
        .unwrap_or("Aldre");
    let flodversion_rad = if andelar.is_some() {
        "\n  \"flodversion\": 2,"
    } else {
        ""
    };
    let extra_lager = if let Some(a) = andelar {
        let foror_enhet = if a.har_vall {
            "fara, bredd_m, balte_m, rutor, vall_m"
        } else {
            "fara, bredd_m, balte_m, rutor"
        };
        let foror_rad = if a.foror.is_empty() {
            String::new()
        } else {
            format!(
                r#",
    "foror": {{"fil": "foror.tsv", "typ": "tsv", "enhet": "{foror_enhet}", "nodata": null, "marke": "simulering"}}"#
            )
        };
        let vall_rad = if a.har_vall {
            format!(
                r#",
    "vall_andel": {{"fil": "vall_andel.f32", "typ": "f32", "enhet": "andel 0..1 av rutan som strandvall", "nodata": null, "marke": "{m_vall}"}},
    "vall_hojd": {{"fil": "vall_hojd.f32", "typ": "f32", "enhet": "m över slätten", "nodata": null, "marke": "{m_vh}"}}"#,
                m_vall = marke_str(a.vall_andel.marke),
                m_vh = marke_str(a.vall_hojd_m.marke),
            )
        } else {
            String::new()
        };
        let vaxt_rad = if a.har_vaxtgrupper {
            format!(
                r#",
    "vaxt_tathet": {{"fil": "vaxt_tathet.f32", "typ": "f32x6", "enhet": "andel 0..1 per växtgrupp (lovtrad, barrtrad, ort, ormbunke, kottepalm_ginkgo, vattenvaxt)", "nodata": null, "marke": "{m_vt}"}},
    "vaxtgrupper": {{"fil": "vaxtgrupper.json", "typ": "json", "enhet": "namn, hojd_m, marke, arter", "nodata": null, "marke": "simulering"}}"#,
                m_vt = marke_str(a.vaxt_tathet.marke),
            )
        } else {
            String::new()
        };
        format!(
            r#",
    "fara_andel": {{"fil": "fara_andel.f32", "typ": "f32", "enhet": "andel 0..1 av rutan som fåra", "nodata": null, "marke": "{m_fara}"}},
    "balte_andel": {{"fil": "balte_andel.f32", "typ": "f32", "enhet": "andel 0..0,85 av rutan som flodbälte", "nodata": null, "marke": "{m_balte}"}}{vall_rad}{vaxt_rad}{foror_rad}"#,
            m_fara = marke_str(a.fara_andel.marke),
            m_balte = marke_str(a.balte_andel.marke),
            vall_rad = vall_rad,
            vaxt_rad = vaxt_rad,
            foror_rad = foror_rad,
        )
    } else {
        String::new()
    };
    format!(
        r#"{{
  "version": 1,{flodversion_rad}
  "omrade": {{
    "lat0": {lat0},
    "lon0": {lon0},
    "b": {b},
    "h": {h},
    "ruta_m": {ruta_m}
  }},
  "t": {t},
  "varde_galler": "rutans mitt (mittpunkten flyttas till 66 miljoner år sedan och läses där)",
  "lager": {{
    "hojd": {{"fil": "hojd.f32", "typ": "f32", "enhet": "m över havsytan", "nodata": null, "marke": "{m_hojd}"}},
    "vatten_yta": {{"fil": "vatten_yta.f32", "typ": "f32", "enhet": "m över havsytan", "nodata": null, "marke": "{m_vy}"}},
    "vattentyp": {{"fil": "vattentyp.u8", "typ": "u8", "enhet": "kod: 0 land, 1 flodfåra, 2 träsk, 3 sjö, 4 bräckt, 5 hav", "nodata": null, "marke": "{m_vt}"}},
    "mark": {{"fil": "mark.f32", "typ": "f32x4", "enhet": "andelar 0..1: sand, silt, lera, torv", "nodata": null, "marke": "{m_mark}"}},
    "vaxt": {{"fil": "vaxt.f32", "typ": "f32", "enhet": "täthet 0..1", "nodata": null, "marke": "{m_vaxt}"}},
    "period": {{"fil": "period.u8", "typ": "u8", "enhet": "kod: 0 äldre, 1 övre, 2 gränsen", "nodata": null, "marke": "{m_period}"}}{extra_lager}
  }},
  "period": "{period}",
  "kontrollsumma": "{summa}",
  "hojd_png": {{
    "fil": "hojd.png",
    "skala": "(hojd_m + 1000) * 10",
    "offset_m": 1000.0,
    "faktor": 10.0
  }},
  "strand": {{"fil": "strand.json"}}
}}
"#,
        flodversion_rad = flodversion_rad,
        lat0 = fmt_f64(o.lat0),
        lon0 = fmt_f64(o.lon0),
        b = o.b,
        h = o.h,
        ruta_m = fmt_f64(o.ruta_m),
        t = fmt_f64(k.t),
        m_hojd = marke_str(k.hojd.marke),
        m_vy = marke_str(k.vatten_yta.marke),
        m_vt = marke_str(k.vattentyp.marke),
        m_mark = marke_str(k.mark.marke),
        m_vaxt = marke_str(k.vaxt.marke),
        m_period = marke_str(k.period.marke),
        extra_lager = extra_lager,
        period = period,
        summa = summa,
    )
}

struct Rubrik {
    omrade: Omrade,
    t: f64,
    flodversion: u8,
    marke_hojd: Marke,
    marke_vatten_yta: Marke,
    marke_vattentyp: Marke,
    marke_mark: Marke,
    marke_vaxt: Marke,
    marke_period: Marke,
    marke_fara_andel: Option<Marke>,
    marke_balte_andel: Option<Marke>,
    marke_vall_andel: Option<Marke>,
    marke_vall_hojd: Option<Marke>,
    marke_vaxt_tathet: Option<Marke>,
    kontrollsumma: u64,
}

fn parsa_karta_json(text: &str) -> Result<Rubrik, String> {
    let version = json_i64(text, "\"version\"")?;
    if version != 1 {
        return Err(format!("okänd version {version}"));
    }
    let flodversion = match json_i64(text, "\"flodversion\"") {
        Ok(v) if v >= 1 && v <= 255 => v as u8,
        Ok(v) => return Err(format!("okänd flodversion {v}")),
        Err(_) => 1,
    };
    let lat0 = json_f64(text, "\"lat0\"")?;
    let lon0 = json_f64(text, "\"lon0\"")?;
    let b = json_i64(text, "\"b\"")? as usize;
    let h = json_i64(text, "\"h\"")? as usize;
    let ruta_m = json_f64(text, "\"ruta_m\"")?;
    let t = json_f64(text, "\"t\"")?;
    let kontrollsumma = json_str_u64(text, "\"kontrollsumma\"")?;

    // Märkena står i lager-objekten; sök efter "hojd": {... "marke": "..." } osv.
    let marke_hojd = marke_fran_str(&json_lager_marke(text, "hojd")?)?;
    let marke_vatten_yta = marke_fran_str(&json_lager_marke(text, "vatten_yta")?)?;
    let marke_vattentyp = marke_fran_str(&json_lager_marke(text, "vattentyp")?)?;
    let marke_mark = marke_fran_str(&json_lager_marke(text, "mark")?)?;
    let marke_vaxt = marke_fran_str(&json_lager_marke(text, "vaxt")?)?;
    let marke_period = marke_fran_str(&json_lager_marke(text, "period")?)?;
    let marke_fara_andel = if flodversion >= 2 {
        Some(marke_fran_str(&json_lager_marke(text, "fara_andel")?)?)
    } else {
        None
    };
    let marke_balte_andel = if flodversion >= 2 {
        Some(marke_fran_str(&json_lager_marke(text, "balte_andel")?)?)
    } else {
        None
    };
    // Vallagren saknas i Alt 1343/1386 — optional.
    let marke_vall_andel = json_lager_marke(text, "vall_andel")
        .ok()
        .map(|s| marke_fran_str(&s))
        .transpose()?;
    let marke_vall_hojd = json_lager_marke(text, "vall_hojd")
        .ok()
        .map(|s| marke_fran_str(&s))
        .transpose()?;
    // Växtgrupper saknas i Alt 1343/1386/1371 — optional.
    let marke_vaxt_tathet = json_lager_marke(text, "vaxt_tathet")
        .ok()
        .map(|s| marke_fran_str(&s))
        .transpose()?;

    if b == 0 || h == 0 {
        return Err("området är tomt: b och h måste vara minst 1".into());
    }

    Ok(Rubrik {
        omrade: Omrade {
            lat0,
            lon0,
            b,
            h,
            ruta_m,
        },
        t,
        flodversion,
        marke_hojd,
        marke_vatten_yta,
        marke_vattentyp,
        marke_mark,
        marke_vaxt,
        marke_period,
        marke_fara_andel,
        marke_balte_andel,
        marke_vall_andel,
        marke_vall_hojd,
        marke_vaxt_tathet,
        kontrollsumma,
    })
}

fn json_efter_nyckel<'a>(text: &'a str, nyckel: &str) -> Result<&'a str, String> {
    let i = text
        .find(nyckel)
        .ok_or_else(|| format!("saknar nyckel {nyckel}"))?;
    let rest = &text[i + nyckel.len()..];
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return Err(format!("saknar ':' efter {nyckel}"));
    }
    Ok(rest[1..].trim_start())
}

fn json_f64(text: &str, nyckel: &str) -> Result<f64, String> {
    let rest = json_efter_nyckel(text, nyckel)?;
    let slut = rest
        .find(|c: char| c == ',' || c == '}' || c == '\n' || c.is_whitespace())
        .unwrap_or(rest.len());
    rest[..slut]
        .parse::<f64>()
        .map_err(|_| format!("kunde inte läsa tal efter {nyckel}"))
}

fn json_i64(text: &str, nyckel: &str) -> Result<i64, String> {
    let rest = json_efter_nyckel(text, nyckel)?;
    let slut = rest
        .find(|c: char| c == ',' || c == '}' || c == '\n' || c.is_whitespace())
        .unwrap_or(rest.len());
    rest[..slut]
        .parse::<i64>()
        .map_err(|_| format!("kunde inte läsa heltal efter {nyckel}"))
}

fn json_str_u64(text: &str, nyckel: &str) -> Result<u64, String> {
    let rest = json_efter_nyckel(text, nyckel)?;
    if !rest.starts_with('"') {
        return Err(format!("{nyckel} ska vara en sträng"));
    }
    let inner = &rest[1..];
    let end = inner
        .find('"')
        .ok_or_else(|| format!("ofullständig sträng för {nyckel}"))?;
    inner[..end]
        .parse::<u64>()
        .map_err(|_| format!("kontrollsumman {:?} är inget u64", &inner[..end]))
}

fn json_lager_marke(text: &str, lager: &str) -> Result<String, String> {
    // Begränsa sökningen till objektet under "lager", så top-level "period" inte stör.
    let lager_start = text
        .find("\"lager\"")
        .ok_or_else(|| "saknar \"lager\"".to_string())?;
    let efter_lager = &text[lager_start..];
    let start_obj = efter_lager
        .find('{')
        .ok_or_else(|| "lager: saknar objekt".to_string())?;
    let obj = &efter_lager[start_obj..];
    let slut = matchande_klammer(obj).ok_or_else(|| "lager: trasigt objekt".to_string())?;
    let lager_block = &obj[..=slut];

    let lager_nyckel = format!("\"{lager}\"");
    let lager_pos = lager_block
        .find(&lager_nyckel)
        .ok_or_else(|| format!("saknar lager {lager}"))?;
    let efter = &lager_block[lager_pos..];
    let start_ett = efter
        .find('{')
        .ok_or_else(|| format!("lager {lager}: saknar objekt"))?;
    let ett = &efter[start_ett..];
    let slut_ett = matchande_klammer(ett).ok_or_else(|| format!("lager {lager}: trasigt objekt"))?;
    let block = &ett[..=slut_ett];
    let marke_rest = json_efter_nyckel(block, "\"marke\"")?;
    if !marke_rest.starts_with('"') {
        return Err(format!("lager {lager}: marke ska vara sträng"));
    }
    let inner = &marke_rest[1..];
    let end = inner
        .find('"')
        .ok_or_else(|| format!("lager {lager}: ofullständig marke"))?;
    Ok(inner[..end].to_string())
}

fn matchande_klammer(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let mut djup = 0i32;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        break;
                    }
                    i += 1;
                }
            }
            b'{' => djup += 1,
            b'}' => {
                djup -= 1;
                if djup == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn skriv_strand_json(path: impl AsRef<Path>, kurvor: &[Vec<(f64, f64)>], ruta_m: f64) -> Result<(), String> {
    let mut s = String::from("{\n  \"enhet\": \"rutor\",\n  \"ruta_m\": ");
    s.push_str(&fmt_f64(ruta_m));
    s.push_str(",\n  \"kurvor\": [\n");
    for (ki, kurva) in kurvor.iter().enumerate() {
        s.push_str("    [");
        for (pi, (x, y)) in kurva.iter().enumerate() {
            if pi > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!("[{}, {}]", fmt_f64(*x), fmt_f64(*y)));
        }
        s.push(']');
        if ki + 1 < kurvor.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ],\n  \"kurvor_m\": [\n");
    for (ki, kurva) in kurvor.iter().enumerate() {
        s.push_str("    [");
        for (pi, (x, y)) in kurva.iter().enumerate() {
            if pi > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!(
                "[{}, {}]",
                fmt_f64(x * ruta_m),
                fmt_f64(y * ruta_m)
            ));
        }
        s.push(']');
        if ki + 1 < kurvor.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    fs::write(path.as_ref(), s).map_err(|e| format!("{}: {e}", path.as_ref().display()))
}

// --- strandlinje hjälp -------------------------------------------------------

fn interpolera_noll(a: (f64, f64), va: f32, b: (f64, f64), vb: f32) -> (f64, f64) {
    let va = va as f64;
    let vb = vb as f64;
    let denom = vb - va;
    let t = if denom.abs() < 1e-20 {
        0.5
    } else {
        (0.0 - va) / denom
    };
    (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))
}

fn punkter_nara(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

fn sammanfoga_segment(segment: Vec<[(f64, f64); 2]>) -> Vec<Vec<(f64, f64)>> {
    let mut kvar: Vec<[(f64, f64); 2]> = segment;
    let mut kurvor: Vec<Vec<(f64, f64)>> = Vec::new();

    while let Some(seg) = kvar.pop() {
        let mut kurva = vec![seg[0], seg[1]];
        loop {
            let mut hittad = false;
            let mut i = 0;
            while i < kvar.len() {
                let s = kvar[i];
                let start = *kurva.first().unwrap();
                let slut = *kurva.last().unwrap();
                if punkter_nara(slut, s[0]) {
                    kurva.push(s[1]);
                    kvar.swap_remove(i);
                    hittad = true;
                    break;
                } else if punkter_nara(slut, s[1]) {
                    kurva.push(s[0]);
                    kvar.swap_remove(i);
                    hittad = true;
                    break;
                } else if punkter_nara(start, s[1]) {
                    kurva.insert(0, s[0]);
                    kvar.swap_remove(i);
                    hittad = true;
                    break;
                } else if punkter_nara(start, s[0]) {
                    kurva.insert(0, s[1]);
                    kvar.swap_remove(i);
                    hittad = true;
                    break;
                } else {
                    i += 1;
                }
            }
            if !hittad {
                break;
            }
        }
        kurvor.push(kurva);
    }
    kurvor
}

// --- PNG (16-bitars gråskala, okomprimerad zlib) -----------------------------

fn hojd_till_u16(h: f32) -> u16 {
    let v = ((f64::from(h) + 1000.0) * 10.0).round();
    if v <= 0.0 {
        0
    } else if v >= 65535.0 {
        65535
    } else {
        v as u16
    }
}

fn bygg_hojd_png(b: usize, h: usize, hojd: &[f32]) -> Result<Vec<u8>, String> {
    if hojd.len() != b * h {
        return Err("höjdlagrets längd stämmer inte".into());
    }
    // Råbild: filterbyte 0 + BE u16 per pixel, radvis j = 0..h (syd→nord).
    let rad_bytes = 1 + b * 2;
    let mut raw = Vec::with_capacity(h * rad_bytes);
    for j in 0..h {
        raw.push(0); // filter None
        for i in 0..b {
            let v = hojd_till_u16(hojd[i + j * b]);
            raw.extend_from_slice(&v.to_be_bytes());
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(b as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.push(16); // bitdjup
    ihdr.push(0); // färgtyp: gråskala
    ihdr.push(0); // kompression
    ihdr.push(0); // filtermetod
    ihdr.push(0); // ingen interlacing
    skriv_chunk(&mut png, b"IHDR", &ihdr);

    let idat = zlib_store(&raw);
    skriv_chunk(&mut png, b"IDAT", &idat);
    skriv_chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

fn skriv_chunk(ut: &mut Vec<u8>, typ: &[u8; 4], data: &[u8]) {
    ut.extend_from_slice(&(data.len() as u32).to_be_bytes());
    ut.extend_from_slice(typ);
    ut.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(typ);
    crc_data.extend_from_slice(data);
    ut.extend_from_slice(&crc32(&crc_data).to_be_bytes());
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    // CMF/FLG: deflate, 32K fönster, kontrollbitar så att (CMF*256+FLG) % 31 == 0.
    // 0x78 0x01 är vanligt för "default"/lägsta kompression.
    let mut ut = vec![0x78, 0x01];
    let mut offset = 0;
    while offset < data.len() || data.is_empty() {
        let kvar = data.len() - offset;
        let n = kvar.min(65535);
        let sista = offset + n >= data.len();
        ut.push(if sista { 0x01 } else { 0x00 }); // BFINAL + BTYPE=00
        let len = n as u16;
        ut.extend_from_slice(&len.to_le_bytes());
        ut.extend_from_slice(&(!len).to_le_bytes());
        ut.extend_from_slice(&data[offset..offset + n]);
        offset += n;
        if data.is_empty() {
            break;
        }
    }
    let a = adler32(data);
    ut.extend_from_slice(&a.to_be_bytes());
    ut
}

fn crc32(data: &[u8]) -> u32 {
    let mut c: u32 = 0xffff_ffff;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            if c & 1 != 0 {
                c = (c >> 1) ^ 0xEDB8_8320;
            } else {
                c >>= 1;
            }
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
