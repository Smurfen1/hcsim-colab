//! `kartvy --fil FIL --val`: rekommendationen för kartvyns öppna val, skriven i terminalen (Håkan 2026-11-22: "Skapa en
//! rekommendation ... På ett visuellt bra sätt. Lättförståeligt", och "Allt ska ju visas i terminalen"). Talen räknas här ur
//! kartbilden; kartbitarna är riktiga utsnitt ur kartan, ritade med kartvyns egna färger och prickar. Alt-numren kommer ur den
//! gemensamma räknaren (sessionen HJARNAN): 1639–1642 och 1649–1650.
use super::*;

const SKALA: usize = 2;
const TRANGSEL: usize = 5 * SKALA * SKALA;
const GRON: (u8, u8, u8) = (124, 196, 143);
const SVAG: (u8, u8, u8) = (150, 156, 142);

/// Siffrorna som valen bygger på, räknade ur kartbilden.
pub struct Lage {
    pub djur: usize,
    pub rutor: usize,
    pub med_djur: usize,
    pub median: usize,
    pub flest: usize,
    pub over16: usize,
    pub vaxt_kvar: f64,
    pub i_sjo: usize,
    pub bla_med_djur: usize,
}

pub fn lage(bild: &Bild) -> Lage {
    let (_, _, r) = rutor(bild, SKALA);
    let mut n: Vec<usize> = r.iter().filter(|q| q.djur > 0).map(|q| q.djur).collect();
    n.sort_unstable();
    let (sv, st) = (0..bild.v.len()).filter(|&c| har_vaxter(bild.vatten[c])).fold((0.0, 0.0), |a, c| (a.0 + bild.v[c], a.1 + bild.vmax[c]));
    Lage {
        djur: bild.djur.len(),
        rutor: r.len(),
        med_djur: n.len(),
        median: if n.is_empty() { 0 } else { n[n.len() / 2] },
        flest: n.last().copied().unwrap_or(0),
        over16: n.iter().filter(|&&x| x > PRICKAR_MAX).count(),
        vaxt_kvar: if st > 0.0 { sv / st } else { 0.0 },
        i_sjo: bild.djur.iter().filter(|d| bild.vatten[d.1 * bild.b + d.0] == 3).count(),
        bla_med_djur: r.iter().filter(|q| q.vatten && q.djur > 0).count(),
    }
}

/// Andelen kartrutor med växter (land, flodkant, träsk) i skärmrutan `r`.
fn landandel(bild: &Bild, r: usize, bb: usize) -> f64 {
    let (x0, y0) = ((r % bb) * SKALA, (r / bb) * SKALA);
    let (mut land, mut alla) = (0, 0);
    for j in y0..(y0 + SKALA).min(bild.h) {
        for i in x0..(x0 + SKALA).min(bild.b) {
            alla += 1;
            land += har_vaxter(bild.vatten[j * bild.b + i]) as usize;
        }
    }
    if alla == 0 { 0.0 } else { land as f64 / alla as f64 }
}

/// Ett utsnitt ur kartan, `b` × `h` skärmrutor från (x0, y0); `per` djur per prick; `mellan` ritar en vattenruta som också har
/// land delad (Alt 1641): vänstra tecknet blått, högra i landets färg. (En mellanton efter andelen land syntes knappt: 1 land
/// av 4 gav (12, 100, 160) mot vattnets (0, 95, 175).)
fn bit(bild: &Bild, x0: usize, y0: usize, b: usize, h: usize, per: usize, mellan: bool) -> Vec<String> {
    let (bb, hh, r) = rutor(bild, SKALA);
    let topp = skal_topp(&r, false);
    let mut ut = Vec::new();
    for y in y0..(y0 + h).min(hh) {
        let mut s = String::new();
        for x in x0..(x0 + b).min(bb) {
            let q = &r[y * bb + x];
            let land = if q.flod > 0 {
                blanda(vaxt_farg(q.vaxt_andel, topp), FLODKANT, BLAND)
            } else if q.trask > 0 {
                blanda(vaxt_farg(q.vaxt_andel, topp), TRASK, BLAND)
            } else {
                vaxt_farg(q.vaxt_andel, topp)
            };
            let delad = q.vatten && mellan && landandel(bild, y * bb + x, bb) > 0.0;
            let (f1, f2) = match (q.vatten, delad) {
                (false, _) => (land, land),
                (true, false) => (VATTEN, VATTEN),
                (true, true) => (VATTEN, land),
            };
            let n = q.djur.div_ceil(per);
            let t: Vec<char> = if q.djur == 0 { vec![' ', ' '] } else { prickar(n).chars().collect() };
            let pf = fg(tal_farg(q.djur, TRANGSEL));
            s.push_str(&format!("{}{pf}{}{}{pf}{}{AV}", bg(f1), t[0], bg(f2), t[1]));
        }
        ut.push(s);
    }
    ut
}

/// Den första blå skärmrutan med djur (x, y), för val 2.
fn bla_ruta(bild: &Bild) -> Option<(usize, usize)> {
    let (bb, _, r) = rutor(bild, SKALA);
    r.iter().position(|q| q.vatten && q.djur > 0).map(|p| (p % bb, p / bb))
}

fn pct(x: f64) -> String {
    format!("{:.1}", 100.0 * x).replace('.', ",")
}
fn tusen(n: usize) -> String {
    let s = n.to_string();
    let mut ut = String::new();
    for (k, c) in s.chars().enumerate() {
        if k > 0 && (s.len() - k) % 3 == 0 {
            ut.push(' ');
        }
        ut.push(c);
    }
    ut
}

/// Två kartbitar sida vid sida med rubriker.
fn par(s: &mut String, va: (&str, bool, Vec<String>), ho: (&str, bool, Vec<String>)) {
    let rub = |t: &str, rek: bool| {
        if rek { format!("{}{FET}✔ {t}{AV}", fg(GRON)) } else { format!("{}  {t}{AV}", fg(SVAG)) }
    };
    let w = va.2.first().map(|l| synlig(l)).unwrap_or(0).max(synlig(va.0) + 2) + 4;
    let r1 = rub(va.0, va.1);
    s.push_str(&format!("   {r1}{}{}\n", " ".repeat(w.saturating_sub(synlig(va.0) + 2)), rub(ho.0, ho.1)));
    for k in 0..va.2.len().max(ho.2.len()) {
        let a = va.2.get(k).cloned().unwrap_or_default();
        let b = ho.2.get(k).cloned().unwrap_or_default();
        s.push_str(&format!("   {a}{}{b}\n", " ".repeat(w.saturating_sub(synlig(&a)))));
    }
}

/// Antal synliga tecken (utan färgkoder).
fn synlig(t: &str) -> usize {
    let mut n = 0;
    let mut i_kod = false;
    for c in t.chars() {
        if c == '\x1b' {
            i_kod = true;
        } else if i_kod {
            if c == 'm' {
                i_kod = false;
            }
        } else {
            n += 1;
        }
    }
    n
}

fn rubrik(s: &mut String, nr: usize, t: &str) {
    s.push_str(&format!("\n{FET}VAL {nr}  {t}{AV}\n"));
}
fn rad(s: &mut String, t: &str) {
    s.push_str(&format!("   {t}\n"));
}
fn alt(s: &mut String, nr: u32, rek: bool, t: &str) {
    if rek {
        s.push_str(&format!("   {}{FET}✔ Alt {nr}{AV} {}(rekommenderas){AV}  {t}\n", fg(GRON), fg(GRON)));
    } else {
        s.push_str(&format!("   {}  Alt {nr}{AV}  {t}\n", fg(SVAG)));
    }
}

pub fn rita_val(bild: &Bild) -> String {
    let l = lage(bild);
    let mut s = String::new();
    s.push_str(&format!("{FET}KARTVYNS FYRA VAL{AV}   {}   år {:.1}\n", bild.rubrik, bild.ar));
    s.push_str(&format!("{}{}{AV}\n", fg(SVAG), "─".repeat(78)));
    rad(&mut s, &format!(
        "Djur {}  ·  skärmrutor med djur {} av {}  ·  vanlig ruta {} djur (median), flest {}",
        tusen(l.djur), l.med_djur, l.rutor, l.median, l.flest
    ));
    rad(&mut s, &format!("Växter kvar {} % av taket  ·  djur i sjörutor {} ({} %)", pct(l.vaxt_kvar), l.i_sjo, pct(l.i_sjo as f64 / l.djur.max(1) as f64)));
    s.push_str(&format!("\n{}{FET}MIN REKOMMENDATION{AV}\n", fg(GRON)));
    rad(&mut s, "1. Behåll en prick per djur (Alt 1649).");
    rad(&mut s, "2. Rita en ruta med både sjö och land delad, halv blå och halv grön (Alt 1641).");
    rad(&mut s, "3. Låt agenterna söka källor om sjöarna (Alt 1642).");
    rad(&mut s, "4. Behåll växterna; färgen är klar (Alt 1639).");

    rubrik(&mut s, 1, "Hur många djur är en prick?");
    rad(&mut s, &format!("En ruta rymmer {PRICKAR_MAX} prickar. Bara {} av {} rutor har fler än {PRICKAR_MAX} djur.", l.over16, l.med_djur));
    let (x0, y0) = (10, 6);
    par(&mut s, ("Alt 1649: en prick per djur", true, bit(bild, x0, y0, 12, 5, 1, false)), ("Alt 1650: en prick per två djur", false, bit(bild, x0, y0, 12, 5, 2, false)));
    alt(&mut s, 1649, true, "Du kan räkna djuren. Så ser kartvyn ut nu.");
    alt(&mut s, 1650, false, "Mer grönt syns, men en prick betyder 1 eller 2 djur.");

    rubrik(&mut s, 2, "Hur ska en ruta med både sjö och land ritas?");
    rad(&mut s, &format!("Med --skala 2 är en skärmruta 2 × 2 kartrutor. {} blå rutor har djur, eftersom de också har land.", l.bla_med_djur));
    if let Some((bx, by)) = bla_ruta(bild) {
        let (x0, y0) = (bx.saturating_sub(4), by.saturating_sub(2));
        par(&mut s, ("I dag: mest vatten blir helt blått", false, bit(bild, x0, y0, 9, 5, 1, false)), ("Alt 1641: delad ruta", true, bit(bild, x0, y0, 9, 5, 1, true)));
        rad(&mut s, &format!("{}(den blå rutan står i mitten av utsnittet; x {}, y {} i kartrutor){AV}", fg(SVAG), bx * SKALA, by * SKALA));
    }
    alt(&mut s, 1641, true, "En ruta med både sjö och land ritas halv blå, halv grön. Ändrar bara kartvyn.");

    rubrik(&mut s, 3, "Får djuren stå i en sjö?");
    rad(&mut s, &format!("I dag får djuren gå in i en sjö mot extra energi. {} djur står nu i sjörutor.", l.i_sjo));
    rad(&mut s, "Ingen läst källa säger hur sjöarna ska fungera (regel 10 kräver en källa).");
    alt(&mut s, 1642, true, "Agenterna söker källor och röstar, t.ex. att djuren bara får passera en sjö.");
    rad(&mut s, &format!("{}  Annars står regeln kvar som en öppen punkt, utan källa.{AV}", fg(SVAG)));

    rubrik(&mut s, 4, "Gällde \"mycket växter\" färgen eller växterna?");
    rad(&mut s, &format!("Djuren har ätit ner växterna till {} % av det största som marken kan bära.", pct(l.vaxt_kvar)));
    alt(&mut s, 1639, true, "Bara färgen. Den är redan ändrad (grönare, sedan mindre grönt och tydlig flodkant).");
    alt(&mut s, 1640, false, "Fler växter i simuleringen. Ändrar världen och kräver en källa om Hell Creeks växter.");
    s.push_str(&format!("\n{}Talen är räknade ur kartbilden nu; kartbitarna är riktiga utsnitt ur kartan.{AV}\n", fg(SVAG)));
    s
}

#[cfg(test)]
mod prov {
    use super::*;
    /// 2 × 2 kartrutor: tre sjöar och en flodkant (som Håkans blå rutor), två djur på flodkanten och ett i en sjö.
    fn sjobild() -> Bild {
        Bild {
            rubrik: "p".into(),
            ar: 1.0,
            markerad: true,
            b: 2,
            h: 2,
            v: vec![0.0, 0.0, 0.5, 0.0],
            vmax: vec![0.0, 0.0, 1.0, 0.0],
            vatten: vec![3, 3, 1, 3],
            djur: vec![(0, 1, true), (0, 1, true), (1, 0, true)],
        }
    }
    #[test]
    fn talen_raknas_ur_kartbilden() {
        let l = lage(&sjobild());
        assert_eq!((l.djur, l.rutor, l.med_djur, l.median, l.flest, l.over16, l.i_sjo, l.bla_med_djur), (3, 1, 1, 3, 3, 0, 1, 1));
        assert!((l.vaxt_kvar - 0.5).abs() < 1e-12);
        let t = rita_val(&sjobild());
        for del in ["Djur 3 ", "djur i sjörutor 1 (33,3 %)", "Växter kvar 50,0 %", "Alt 1649", "Alt 1641", "Alt 1642", "Alt 1639"] {
            assert!(t.contains(del), "saknar {del:?}");
        }
    }
    #[test]
    fn delad_ruta_visar_bade_vatten_och_land() {
        let b = sjobild();
        let i_dag = bit(&b, 0, 0, 1, 1, 1, false).join("");
        let delad = bit(&b, 0, 0, 1, 1, 1, true).join("");
        assert_eq!(i_dag.matches(&bg(VATTEN)).count(), 2, "i dag är rutan helt blå: {i_dag:?}");
        assert_eq!(delad.matches(&bg(VATTEN)).count(), 1, "den delade rutan har en blå halva: {delad:?}");
        assert_eq!(synlig(&delad), 2, "rutan är två tecken bred");
    }
    #[test]
    fn synlig_bredd_utan_fargkoder() {
        assert_eq!(synlig(&format!("{}ab{AV}c", bg(VATTEN))), 3);
    }
}
