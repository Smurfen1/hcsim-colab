//! Källtabellsläsare och spärrar för övriga sjukdomar i Hell Creek (Alt 2216 + Alt 2217;
//! Håkans byggbeslut Alt 2220; protokoll `n10/docs/2026-11-24-omrostning-sjukdomar.md`).
//!
//! Viktigt (Alt 2216): inga nya pixel- eller rutlager byggs i `hckarta` för sjukdomar eller vektorer.
//! Kartans befintliga fysiska lager (`Flodesklass`, `avstand_vatten_m`, `FlodZon`, `BasUnderlag`)
//! ger hela det geografiska underlaget, medan smittspridning, sårinfektioner, kadaverreservoarer,
//! täthet och säsong ägs av BRAIN i `hcsim` (regel 14 och 15).

/// En rad ur den låsta källtabellen `sjukdomar.tsv` (13 kolumner, samma modell som
/// `naturhandelser.tsv` och `forna_alger_plankton.tsv`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SjukdomsRad {
    pub kategori: String,
    pub smittvag: String,
    pub matt: String,
    pub varde: String,
    pub enhet: String,
    pub kalla: String,
    pub doi: String,
    pub citat: String,
    pub last: String,
    pub plats_tid: String,
    pub ursprung: String,
    pub markning: String,
    pub forbehall: String,
}

const RUBRIK: &str =
    "kategori\tsmittvag\tmatt\tvarde\tenhet\tkalla\tdoi\tcitat\tlast\tplats_tid\tursprung\tmarkning\tforbehall";

/// Läser och kontrollerar den låsta källtabellen `sjukdomar.tsv` med hårda spärrar (Alt 2216 + Alt 2217).
pub fn las_sjukdomar_tsv(text: &str) -> Result<Vec<SjukdomsRad>, String> {
    let mut rader = Vec::new();
    let mut sett_rubrik = false;

    for (rad_nr, ra) in text.lines().enumerate() {
        let rad = ra.trim_end_matches('\r');
        if rad.trim().is_empty() || rad.starts_with('#') {
            continue;
        }
        if !sett_rubrik {
            if rad != RUBRIK {
                return Err(format!(
                    "sjukdomar.tsv rad {}: oväntad rubrik {:?}",
                    rad_nr + 1,
                    rad
                ));
            }
            sett_rubrik = true;
            continue;
        }

        let f: Vec<&str> = rad.split('\t').collect();
        if f.len() != 13 {
            return Err(format!(
                "sjukdomar.tsv rad {}: väntade 13 kolumner, fick {}",
                rad_nr + 1,
                f.len()
            ));
        }

        let (
            kategori,
            smittvag,
            matt,
            varde,
            enhet,
            kalla,
            doi,
            citat,
            last,
            plats_tid,
            ursprung,
            markning,
            forbehall,
        ) = (
            f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11], f[12],
        );

        if !matches!(last, "fulltext" | "sammandrag" | "fulltext_del" | "ej_last") {
            return Err(format!(
                "sjukdomar.tsv rad {}: ogiltigt värde för last: {last:?}",
                rad_nr + 1
            ));
        }
        if !matches!(
            ursprung,
            "hell_creek" | "grannformation" | "aldre" | "globalt" | "nutid"
        ) {
            return Err(format!(
                "sjukdomar.tsv rad {}: ogiltigt ursprung: {ursprung:?}",
                rad_nr + 1
            ));
        }
        if !matches!(markning, "Hell Creek" | "lanat" | "eget val" | "ingen_kalla") {
            return Err(format!(
                "sjukdomar.tsv rad {}: ogiltig märkning: {markning:?}",
                rad_nr + 1
            ));
        }
        if forbehall.trim().is_empty() || forbehall == "-" {
            return Err(format!(
                "sjukdomar.tsv rad {}: saknar förbehåll",
                rad_nr + 1
            ));
        }

        // Spärr 1 (Alt 2217 / regel 16): ej_last eller ingen_kalla får aldrig bära ett tal/värde.
        if last == "ej_last" || markning == "ingen_kalla" {
            if varde != "-" {
                return Err(format!(
                    "sjukdomar.tsv rad {}: värde {varde:?} utan läst källa (last={last}, markning={markning})",
                    rad_nr + 1
                ));
            }
        } else {
            if !doi.starts_with("10.") {
                return Err(format!(
                    "sjukdomar.tsv rad {}: saknar giltig DOI (fick {doi:?})",
                    rad_nr + 1
                ));
            }
            if citat.trim().is_empty() || citat == "-" {
                return Err(format!(
                    "sjukdomar.tsv rad {}: saknar ordagrant citat",
                    rad_nr + 1
                ));
            }
        }

        let kat_l = kategori.to_lowercase();
        let matt_l = matt.to_lowercase();
        let plats_l = plats_tid.to_lowercase();
        let cit_l = citat.to_lowercase();

        // Spärr 2 (Alt 2217): ingen dubbelräkning av vattenburna cyanotoxiner/botulism här.
        if (kat_l.contains("cyanotoxin")
            || kat_l.contains("botulism")
            || matt_l.contains("cyanotoxin")
            || matt_l.contains("botulism"))
            && matt != "sparr_vattenburna_dubbelrakning"
        {
            return Err(format!(
                "sjukdomar.tsv rad {}: vattenburen dubbelräkning ({kategori}/{matt}) tillhör vattenkvalitet.tsv",
                rad_nr + 1
            ));
        }

        // Spärr 3 (Alt 2217): moderna däggdjurs-/fågelvirus utan kritabelägg får inte läggas in som aktiva rader.
        if (kat_l.contains("influensa")
            || kat_l.contains("morbillivirus")
            || kat_l.contains("west nile")
            || matt_l.contains("influensa")
            || matt_l.contains("morbillivirus")
            || matt_l.contains("west_nile"))
            && matt != "sparr_moderna_virus"
        {
            return Err(format!(
                "sjukdomar.tsv rad {}: modernt virus utan kritabelägg spärrat ({kategori}/{matt})",
                rad_nr + 1
            ));
        }

        // Spärr 4 (Alt 2217): äldre/nutida fynd och B. anthracis får aldrig märkas som hell_creek.
        if ursprung == "hell_creek" {
            for ord in [
                "myanmar",
                "bärnsten",
                "morrison",
                "brasilien",
                "zimbabwe",
                "namibia",
                "anthracis",
                "nutida",
            ] {
                if plats_l.contains(ord) || cit_l.contains(ord) {
                    return Err(format!(
                        "sjukdomar.tsv rad {}: rad som nämner {ord:?} får inte ha ursprung=hell_creek",
                        rad_nr + 1
                    ));
                }
            }
            if varde == "32.0|53.9" {
                return Err(format!(
                    "sjukdomar.tsv rad {}: nutida jämförelsetal 32.0|53.9 får inte märkas hell_creek",
                    rad_nr + 1
                ));
            }
        }

        rader.push(SjukdomsRad {
            kategori: kategori.to_string(),
            smittvag: smittvag.to_string(),
            matt: matt.to_string(),
            varde: varde.to_string(),
            enhet: enhet.to_string(),
            kalla: kalla.to_string(),
            doi: doi.to_string(),
            citat: citat.to_string(),
            last: last.to_string(),
            plats_tid: plats_tid.to_string(),
            ursprung: ursprung.to_string(),
            markning: markning.to_string(),
            forbehall: forbehall.to_string(),
        });
    }

    if !sett_rubrik || rader.is_empty() {
        return Err("sjukdomar.tsv saknar datarader".into());
    }
    Ok(rader)
}
