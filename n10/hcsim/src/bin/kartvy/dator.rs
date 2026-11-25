//! En rad om en annan dators läge i kartvyns fönster (Håkans Alt 1714, 2026-11-22: "Du ska visa data om N10-datorn i samma
//! terminalfönster"): last, minne, processorns temperatur och om simuleringen kör där. Läses över ssh var 10:e sekund i en
//! egen tråd, så att kartvyn aldrig väntar på nätet. Kartvyn läser bara; inget ändras på den andra datorn.

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Kommandot som körs på den andra datorn. Varje rad börjar med ett nyckelord, så att tolkningen inte beror på ordningen.
pub const FRAGA: &str = "echo last $(cut -d' ' -f1 /proc/loadavg); echo karnor $(nproc); \
awk '/^MemTotal:|^MemAvailable:/{print \"minne\", $1, $2}' /proc/meminfo; \
for z in /sys/class/thermal/thermal_zone*; do echo zon $(cat $z/type 2>/dev/null) $(cat $z/temp 2>/dev/null); done; \
echo hcsim $(pgrep -c hcsim)";

#[derive(Debug, Clone, PartialEq)]
pub struct Dator {
    pub last: f64,
    pub karnor: usize,
    pub minne_total_kb: u64,
    pub minne_ledigt_kb: u64,
    /// Processorns temperatur i grader (givaren x86_pkg_temp); `None` om givaren saknas.
    pub temp_c: Option<f64>,
    pub hcsim: usize,
}

/// Tolkar svaret. `None` om lasten, kärnorna eller minnet saknas (ett halvt svar visas aldrig som ett läge).
pub fn tolka(ut: &str) -> Option<Dator> {
    let (mut last, mut karnor, mut tot, mut led, mut temp, mut hcsim) = (None, None, None, None, None, None);
    for rad in ut.lines() {
        let d: Vec<&str> = rad.split_whitespace().collect();
        match d.as_slice() {
            ["last", x] => last = x.parse::<f64>().ok(),
            ["karnor", x] => karnor = x.parse::<usize>().ok(),
            ["minne", "MemTotal:", x] => tot = x.parse::<u64>().ok(),
            ["minne", "MemAvailable:", x] => led = x.parse::<u64>().ok(),
            ["zon", "x86_pkg_temp", x] => temp = x.parse::<f64>().ok().map(|m| m / 1000.0),
            ["hcsim", x] => hcsim = x.parse::<usize>().ok(),
            _ => {}
        }
    }
    let (minne_total_kb, minne_ledigt_kb) = (tot?, led?);
    if minne_ledigt_kb > minne_total_kb {
        return None;
    }
    Some(Dator { last: last?, karnor: karnor.filter(|&k| k > 0)?, minne_total_kb, minne_ledigt_kb, temp_c: temp, hcsim: hcsim.unwrap_or(0) })
}

fn komma(x: f64, dec: usize) -> String {
    format!("{x:.dec$}").replace('.', ",")
}

/// Raden som visas. `lage` är det senaste lästa läget och hur gammalt det är; äldre än `max_alder` räknas som ingen kontakt.
pub fn rad(namn: &str, lage: Option<(&Dator, Duration)>, max_alder: Duration) -> String {
    let n = namn.to_uppercase();
    match lage {
        Some((d, alder)) if alder <= max_alder => {
            let gb = |kb: u64| kb as f64 / 1_048_576.0;
            let temp = d.temp_c.map(|t| format!("{} °C", komma(t, 0))).unwrap_or_else(|| "temperatur saknas".into());
            let sim = if d.hcsim > 0 { "simuleringen kör" } else { "simuleringen kör inte" };
            format!(
                "Datorn {n}: last {} av {} kärnor · minne {} av {} GB använt · processorn {temp} · {sim}\n",
                komma(d.last, 2),
                d.karnor,
                komma(gb(d.minne_total_kb - d.minne_ledigt_kb), 1),
                komma(gb(d.minne_total_kb), 1),
            )
        }
        _ => format!("Datorn {n}: ingen kontakt\n"),
    }
}

/// Det delade läget: senaste svaret och när det lästes.
pub type Delat = Arc<Mutex<Option<(Dator, Instant)>>>;

/// Frågar datorn en gång över ssh (utan lösenordsfråga, högst 5 s för anslutningen).
pub fn fraga(namn: &str) -> Option<Dator> {
    let ut = Command::new("ssh")
        .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", namn, FRAGA])
        .output()
        .ok()?;
    tolka(&String::from_utf8_lossy(&ut.stdout))
}

/// Startar tråden som frågar datorn var `var_s` sekund.
pub fn starta(namn: String, var_s: u64) -> Delat {
    let delat: Delat = Arc::new(Mutex::new(None));
    let d = delat.clone();
    std::thread::spawn(move || loop {
        if let Some(svar) = fraga(&namn) {
            if let Ok(mut l) = d.lock() {
                *l = Some((svar, Instant::now()));
            }
        }
        std::thread::sleep(Duration::from_secs(var_s.max(1)));
    });
    delat
}

#[cfg(test)]
mod prov {
    use super::*;

    /// Ett riktigt svar från N10 (läst 2026-11-22 över ssh), med tre temperaturgivare.
    const SVAR: &str = "last 0.12\nkarnor 4\nminne MemTotal: 16115860\nminne MemAvailable: 13523456\nzon acpitz 27800\nzon TCPU_PCI 57000\nzon x86_pkg_temp 53000\nhcsim 0\n";

    #[test]
    fn svaret_tolkas() {
        let d = tolka(SVAR).unwrap();
        assert_eq!(d, Dator { last: 0.12, karnor: 4, minne_total_kb: 16115860, minne_ledigt_kb: 13523456, temp_c: Some(53.0), hcsim: 0 });
        let r = rad("n10", Some((&d, Duration::from_secs(3))), Duration::from_secs(60));
        assert_eq!(r, "Datorn N10: last 0,12 av 4 kärnor · minne 2,5 av 15,4 GB använt · processorn 53 °C · simuleringen kör inte\n");
    }

    /// Processorns givare gäller, inte moderkortets (acpitz visade 27,8 grader när processorn hade 53).
    #[test]
    fn processorns_givare_valjs() {
        let d = tolka(&SVAR.replace("zon x86_pkg_temp 53000\n", "")).unwrap();
        assert_eq!(d.temp_c, None);
        assert!(rad("n10", Some((&d, Duration::ZERO)), Duration::from_secs(60)).contains("temperatur saknas"));
    }

    #[test]
    fn simuleringen_syns_nar_den_kor() {
        let d = tolka(&SVAR.replace("hcsim 0", "hcsim 1")).unwrap();
        assert!(rad("n10", Some((&d, Duration::ZERO)), Duration::from_secs(60)).contains("· simuleringen kör\n"));
    }

    /// Ett halvt eller tomt svar och ett gammalt läge visas som ingen kontakt, aldrig som ett läge.
    #[test]
    fn halvt_svar_och_gammalt_lage_ar_ingen_kontakt() {
        assert_eq!(tolka(""), None);
        assert_eq!(tolka("last 0.12\nkarnor 4\n"), None);
        assert_eq!(tolka(&SVAR.replace("karnor 4", "karnor 0")), None);
        assert_eq!(tolka(&SVAR.replace("MemAvailable: 13523456", "MemAvailable: 99999999")), None);
        let d = tolka(SVAR).unwrap();
        let ingen = "Datorn N10: ingen kontakt\n";
        assert_eq!(rad("n10", None, Duration::from_secs(60)), ingen);
        assert_eq!(rad("n10", Some((&d, Duration::from_secs(61))), Duration::from_secs(60)), ingen);
    }
}
