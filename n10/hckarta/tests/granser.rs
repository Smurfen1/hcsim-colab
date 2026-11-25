// Claudes prov (2026-11-21): perioderna precis vid tidtabellens gränser (mutanten ovre_grans_fel såg inte S2:s prov).
use hckarta::{Period, Tidtabell};
#[test]
fn perioderna_vid_granserna() {
    let tt = Tidtabell::las(include_str!("../tidtabell.tsv")).unwrap();
    let mv = tt.rader.iter().find(|r| r.handelse == "magnetvandning").and_then(|r| r.ar_fore_gransen).expect("magnetvändningen saknas");
    assert_eq!(tt.period(mv).unwrap(), Period::Ovre);
    assert_eq!(tt.period(mv + 1.0).unwrap(), Period::Aldre);
    assert_eq!(tt.period(mv - 1.0).unwrap(), Period::Ovre);
    assert_eq!(tt.period(1.0).unwrap(), Period::Ovre);
    assert_eq!(tt.period(0.0).unwrap(), Period::Gransen);
    assert_eq!(tt.period(1_081_000.0).unwrap(), Period::Aldre);
}
