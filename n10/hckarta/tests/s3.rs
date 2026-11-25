//! Fem namngivna prov för S3: skriv/läs 3D-filer, PNG och strandlinje.

use std::path::PathBuf;

use hckarta::{
    karta_med, kontrollsumma, las_3d, skriv_3d, strandlinje, Kalla, Karta, Lager, Marke, Omrade,
    Period, Tidtabell, Vatten,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data")
}

fn kalla() -> Kalla {
    Kalla::las(&data()).expect("kunde inte läsa paleokarta/data")
}

fn tidtabell() -> Tidtabell {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv"))
        .expect("kunde inte läsa tidtabell.tsv");
    Tidtabell::las(&text).expect("tidtabellen")
}

fn temp_mapp(namn: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "hckarta-s3-{}-{}-{}",
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

/// En karta med tidtabell skrivs och läses tillbaka; kontrollsumman och alla lager är lika.
#[test]
fn skriv_och_las_samma_summa() {
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 16,
        h: 12,
        ruta_m: 2000.0,
    };
    let kart = karta_med(&kalla(), &tidtabell(), &omr, 500_000.0).unwrap();
    let fore = kontrollsumma(&kart);
    let mapp = temp_mapp("rundresa");
    let skriven = skriv_3d(&kart, &mapp).unwrap();
    assert_eq!(skriven, fore);
    let last = las_3d(&mapp).unwrap();
    assert_eq!(kontrollsumma(&last), fore);
    assert_eq!(last.omrade.b, kart.omrade.b);
    assert_eq!(last.omrade.h, kart.omrade.h);
    assert_eq!(last.t, kart.t);
    assert_eq!(last.hojd.varden, kart.hojd.varden);
    assert_eq!(last.hojd.marke, kart.hojd.marke);
    assert_eq!(last.vatten_yta.varden, kart.vatten_yta.varden);
    assert_eq!(last.vatten_yta.marke, kart.vatten_yta.marke);
    assert_eq!(last.vattentyp.varden, kart.vattentyp.varden);
    assert_eq!(last.vattentyp.marke, kart.vattentyp.marke);
    assert_eq!(last.mark.varden, kart.mark.varden);
    assert_eq!(last.mark.marke, kart.mark.marke);
    assert_eq!(last.vaxt.varden, kart.vaxt.varden);
    assert_eq!(last.vaxt.marke, kart.vaxt.marke);
    assert_eq!(last.period.varden, kart.period.varden);
    assert_eq!(last.period.marke, kart.period.marke);
    let _ = std::fs::remove_dir_all(&mapp);
}

/// Ändras en enda byte i hojd.f32 eller vaxt.f32 ger las_3d Err.
#[test]
fn andrad_byte_ger_fel() {
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 4,
        h: 3,
        ruta_m: 2000.0,
    };
    let kart = karta_med(&kalla(), &tidtabell(), &omr, 500_000.0).unwrap();

    let mapp1 = temp_mapp("byte-hojd");
    skriv_3d(&kart, &mapp1).unwrap();
    let path = mapp1.join("hojd.f32");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 0x01;
    std::fs::write(&path, &bytes).unwrap();
    assert!(las_3d(&mapp1).is_err());
    let _ = std::fs::remove_dir_all(&mapp1);

    let mapp2 = temp_mapp("byte-vaxt");
    skriv_3d(&kart, &mapp2).unwrap();
    let path = mapp2.join("vaxt.f32");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 0x01;
    std::fs::write(&path, &bytes).unwrap();
    assert!(las_3d(&mapp2).is_err());
    let _ = std::fs::remove_dir_all(&mapp2);
}

/// En avkortad period.u8 ger Err.
#[test]
fn fel_langd_ger_fel() {
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 4,
        h: 3,
        ruta_m: 2000.0,
    };
    let kart = karta_med(&kalla(), &tidtabell(), &omr, 500_000.0).unwrap();
    let mapp = temp_mapp("avkortad");
    skriv_3d(&kart, &mapp).unwrap();
    let path = mapp.join("period.u8");
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() > 1);
    std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(las_3d(&mapp).is_err());
    let _ = std::fs::remove_dir_all(&mapp);
}

/// hojd.png börjar med PNG-signaturen, har IHDR med rätt b/h, bitdjup 16, färgtyp 0, och CRC stämmer.
#[test]
fn png_ar_giltig() {
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 8,
        h: 6,
        ruta_m: 2000.0,
    };
    let kart = karta_med(&kalla(), &tidtabell(), &omr, 500_000.0).unwrap();
    let mapp = temp_mapp("png");
    skriv_3d(&kart, &mapp).unwrap();
    let png = std::fs::read(mapp.join("hojd.png")).unwrap();
    assert!(png.len() > 8);
    assert_eq!(&png[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut offset = 8usize;
    while offset + 12 <= png.len() {
        let len = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        let typ = &png[offset + 4..offset + 8];
        let data_start = offset + 8;
        let data_end = data_start + len;
        assert!(data_end + 4 <= png.len(), "chunk kapad");
        let crc_lagrad = u32::from_be_bytes(png[data_end..data_end + 4].try_into().unwrap());
        let mut crc_indata = Vec::with_capacity(4 + len);
        crc_indata.extend_from_slice(typ);
        crc_indata.extend_from_slice(&png[data_start..data_end]);
        assert_eq!(crc32_ref(&crc_indata), crc_lagrad, "CRC för {:?}", std::str::from_utf8(typ));

        if typ == b"IHDR" {
            assert_eq!(len, 13);
            let w = u32::from_be_bytes(png[data_start..data_start + 4].try_into().unwrap());
            let h = u32::from_be_bytes(png[data_start + 4..data_start + 8].try_into().unwrap());
            assert_eq!(w, omr.b as u32);
            assert_eq!(h, omr.h as u32);
            assert_eq!(png[data_start + 8], 16); // bitdjup
            assert_eq!(png[data_start + 9], 0); // färgtyp gråskala
        }
        offset = data_end + 4;
        if typ == b"IEND" {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&mapp);
}

fn crc32_ref(data: &[u8]) -> u32 {
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

/// Västra −10 / östra +10: strandlinjen på x = 2,0; bara land ger tom lista.
#[test]
fn strandlinjen() {
    let n = 4 * 4;
    let mut hojd = vec![10.0f32; n];
    for j in 0..4 {
        for i in 0..4 {
            hojd[i + j * 4] = if i < 2 { -10.0 } else { 10.0 };
        }
    }
    let kart = Karta {
        omrade: Omrade {
            lat0: 47.0,
            lon0: -108.0,
            b: 4,
            h: 4,
            ruta_m: 2000.0,
        },
        t: 0.0,
        hojd: Lager {
            varden: hojd,
            marke: Marke::Simulering,
        },
        vatten_yta: Lager {
            varden: vec![0.0; n],
            marke: Marke::Simulering,
        },
        vattentyp: Lager {
            varden: vec![Vatten::Land; n],
            marke: Marke::Simulering,
        },
        mark: Lager {
            varden: vec![[0.25; 4]; n],
            marke: Marke::Simulering,
        },
        vaxt: Lager {
            varden: vec![0.5; n],
            marke: Marke::Simulering,
        },
        period: Lager {
            varden: vec![Period::Aldre; n],
            marke: Marke::Simulering,
        },
    };
    let kurvor = strandlinje(&kart);
    assert!(!kurvor.is_empty(), "väntade minst en strandkurva");
    for kurva in &kurvor {
        for &(x, _y) in kurva {
            assert!(
                (x - 2.0).abs() < 1e-9,
                "punkt på strandlinjen har x = {x}, väntade 2.0"
            );
        }
    }

    let land = Karta {
        hojd: Lager {
            varden: vec![10.0; n],
            marke: Marke::Simulering,
        },
        ..kart
    };
    assert!(strandlinje(&land).is_empty());
}
