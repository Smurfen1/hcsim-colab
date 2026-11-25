//! Sex namngivna prov för S2: tidtabellens läsare och periodlagret.

use std::path::PathBuf;

use hckarta::{
    karta, karta_med, kontrollsumma, Kalla, Marke, Omrade, Period, Tidtabell, Vatten,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data")
}

fn kalla() -> Kalla {
    Kalla::las(&data()).expect("kunde inte läsa paleokarta/data")
}

fn tidtabell_tsv() -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv"))
        .expect("kunde inte läsa tidtabell.tsv")
}

fn omrade_litet() -> Omrade {
    Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 4,
        h: 3,
        ruta_m: 2000.0,
    }
}

/// Den riktiga tidtabell.tsv läses; den har 9 rader, varav 7 med märket läst och 6 med känd tid.
#[test]
fn tidtabellen_las() {
    let tt = Tidtabell::las(&tidtabell_tsv()).unwrap();
    assert_eq!(tt.rader.len(), 9);
    let last = tt.rader.iter().filter(|r| r.marke == Marke::Last).count();
    assert_eq!(last, 7);
    let med_tid = tt.rader.iter().filter(|r| r.ar_fore_gransen.is_some()).count();
    assert_eq!(med_tid, 6);
}

/// t = 500 000 ger Aldre, t = 100 000 ger Ovre, t = 0 ger Gransen (med den riktiga tabellen).
#[test]
fn perioderna() {
    let tt = Tidtabell::las(&tidtabell_tsv()).unwrap();
    assert_eq!(tt.period(500_000.0).unwrap(), Period::Aldre);
    assert_eq!(tt.period(100_000.0).unwrap(), Period::Ovre);
    assert_eq!(tt.period(0.0).unwrap(), Period::Gransen);
}

/// Raden "magnetvandning" med märket simulering får inte styra; extra simuleringsrad ändrar inte perioden.
#[test]
fn simulering_andrar_inget() {
    let text = tidtabell_tsv().replace(
        "290000\tmagnetvandning\tC30n/C29r; period: övre börjar (Sprains ålder i Montana; Roberts omräkning 66,48)\tSprain m.fl., citerad i Roberts m.fl. 2026\tläst",
        "290000\tmagnetvandning\tC30n/C29r; period: övre börjar (Sprains ålder i Montana; Roberts omräkning 66,48)\tSprain m.fl., citerad i Roberts m.fl. 2026\tsimulering",
    );
    let tt = Tidtabell::las(&text).unwrap();
    assert!(tt.period(100_000.0).is_err());

    let mut extra = tidtabell_tsv();
    extra.push_str("\n150000\textra_sim\tnågot\tting\tsimulering\n");
    let tt2 = Tidtabell::las(&extra).unwrap();
    let bas = Tidtabell::las(&tidtabell_tsv()).unwrap();
    assert_eq!(tt2.period(100_000.0).unwrap(), bas.period(100_000.0).unwrap());
    assert_eq!(tt2.period(500_000.0).unwrap(), bas.period(500_000.0).unwrap());
}

/// En tabell där tiderna för "magnetvandning" och "basen" byter plats ger annan kontrollsumma vid t = 500 000.
#[test]
fn omkastad_tabell_annan_karta() {
    let k = kalla();
    let omr = omrade_litet();
    let tt = Tidtabell::las(&tidtabell_tsv()).unwrap();
    let kart = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let summa = kontrollsumma(&kart);

    // Byt tiderna: magnetvandning får 1081000, basen får 290000.
    let omkastad = tidtabell_tsv()
        .replacen("1081000\tbasen\t", "290000\tbasen\t", 1)
        .replacen("290000\tmagnetvandning\t", "1081000\tmagnetvandning\t", 1);
    let tt2 = Tidtabell::las(&omkastad).unwrap();
    let kart2 = karta_med(&k, &tt2, &omr, 500_000.0).unwrap();
    assert_ne!(kontrollsumma(&kart2), summa);
    assert_eq!(kart.period.varden[0], Period::Aldre);
    assert_eq!(kart2.period.varden[0], Period::Ovre);
}

/// En tabell med bara simuleringsrader, och en helt tom tabell, ger Err.
#[test]
fn tom_tabell_ar_fel() {
    let bara_sim = "\
1000\ta\tb\tc\tsimulering
2000\td\te\tf\tsimulering
";
    assert!(Tidtabell::las(bara_sim).is_err());
    assert!(Tidtabell::las("").is_err());
    assert!(Tidtabell::las("# bara kommentar\n").is_err());
}

/// För vart och ett av lagren: ändras ett enda värde (eller märket) blir kontrollsumman en annan.
#[test]
fn summan_ser_varje_lager() {
    let k = kalla();
    let omr = omrade_litet();
    let tt = Tidtabell::las(&tidtabell_tsv()).unwrap();
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let fore = kontrollsumma(&bas);

    {
        let mut k = bas.clone();
        k.hojd.varden[0] += 1.0;
        assert_ne!(kontrollsumma(&k), fore, "höjd värde");
    }
    {
        let mut k = bas.clone();
        k.hojd.marke = Marke::Simulering;
        assert_ne!(kontrollsumma(&k), fore, "höjd märke");
    }
    {
        let mut k = bas.clone();
        k.vatten_yta.varden[0] += 1.0;
        assert_ne!(kontrollsumma(&k), fore, "vatten_yta värde");
    }
    {
        let mut k = bas.clone();
        k.vatten_yta.marke = Marke::Simulering;
        assert_ne!(kontrollsumma(&k), fore, "vatten_yta märke");
    }
    {
        let mut k = bas.clone();
        k.vattentyp.varden[0] = Vatten::Flodfara;
        assert_ne!(kontrollsumma(&k), fore, "vattentyp värde");
    }
    {
        let mut k = bas.clone();
        k.vattentyp.marke = Marke::Simulering;
        assert_ne!(kontrollsumma(&k), fore, "vattentyp märke");
    }
    {
        let mut k = bas.clone();
        k.mark.varden[0][0] += 0.1;
        assert_ne!(kontrollsumma(&k), fore, "mark värde");
    }
    {
        let mut k = bas.clone();
        k.mark.marke = Marke::Last;
        assert_ne!(kontrollsumma(&k), fore, "mark märke");
    }
    {
        let mut k = bas.clone();
        k.vaxt.varden[0] += 0.1;
        assert_ne!(kontrollsumma(&k), fore, "växt värde");
    }
    {
        let mut k = bas.clone();
        k.vaxt.marke = Marke::Last;
        assert_ne!(kontrollsumma(&k), fore, "växt märke");
    }
    {
        let mut k = bas.clone();
        k.period.varden[0] = Period::Ovre;
        assert_ne!(kontrollsumma(&k), fore, "period värde");
    }
    {
        let mut k = bas.clone();
        k.period.marke = Marke::Simulering;
        assert_ne!(kontrollsumma(&k), fore, "period märke");
    }

    // karta utan tidtabell har period Simulering/Aldre — summan skiljer sig från karta_med.
    let utan = karta(&k, &omr, 500_000.0).unwrap();
    assert_eq!(utan.period.marke, Marke::Simulering);
    assert_eq!(utan.period.varden[0], Period::Aldre);
    assert_ne!(kontrollsumma(&utan), fore);
}
