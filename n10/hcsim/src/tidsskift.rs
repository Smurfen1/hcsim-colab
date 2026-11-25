//! Tidsskiftets databas (Håkan 2026-11-22: "Du ska inte spara bilderna, du ska spara värdena. I en databas, att det inte
//! tar så mycket plats."). En SQLite-fil `FIL.tidsskift.db` bredvid kartbilden.
//!
//! - Det som är lika i alla lägen (rubrik, bredd, höjd, rutornas tak och vatten) sparas en gång, i tabellen `fast`.
//! - Varje läge sparar bara värdena som ändras: klockan, steget, om märkningen är gjord, växtmassan per ruta (f32) och
//!   djuren (två byte per djur: kolumn och rad; märkningen ligger i radens högsta bit).
//! - Databasen har ett tak på `MAX_LAGEN` lägen. När taket nås tas vartannat läge bort (det första och det sista behålls),
//!   och skrivaren ber om dubbelt så lång tid mellan lägena. Storleken växer därför inte med körningens längd; tidsskiftet
//!   går alltid från start till nu, glesare ju längre körningen har gått.
//!
//! `Lasare::lage_json` bygger samma text som den gamla raden i `.historik.jsonl`, så att kartvyn läser den som förut.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};

/// Högst så här många lägen i databasen (eget val; uppspelningen visar högst 120 av dem).
pub const MAX_LAGEN: usize = 600;

/// Databasens fil bredvid kartbilden.
pub fn db_vag(ut: &Path) -> PathBuf {
    let mut s = ut.as_os_str().to_owned();
    s.push(".tidsskift.db");
    PathBuf::from(s)
}

/// Det som är lika i alla lägen.
#[derive(Clone, Debug, PartialEq)]
pub struct Fast {
    pub rubrik: String,
    pub b: usize,
    pub h: usize,
    pub steg_per_ar: i64,
    pub vmax: Vec<f32>,
    pub vatten: Vec<u8>,
}

/// Ett läge: värdena som ändras.
#[derive(Clone, Debug, PartialEq)]
pub struct Lage {
    pub klocka: u64,
    pub t: i64,
    pub markerad: bool,
    pub v: Vec<f32>,
    /// (kolumn, rad, märkt) per levande djur.
    pub djur: Vec<(u8, u8, bool)>,
}

fn f32_till_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn blob_till_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn djur_till_blob(d: &[(u8, u8, bool)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(d.len() * 2);
    for &(i, j, m) in d {
        assert!(j < 128, "raden {j} ryms inte i sju bitar");
        b.push(i);
        b.push(j | if m { 0x80 } else { 0 });
    }
    b
}

fn blob_till_djur(b: &[u8]) -> Vec<(u8, u8, bool)> {
    b.chunks_exact(2).map(|c| (c[0], c[1] & 0x7F, c[1] & 0x80 != 0)).collect()
}

fn fel<E: std::fmt::Display>(vad: &str) -> impl Fn(E) -> String + '_ {
    move |e| format!("tidsskiftets databas ({vad}): {e}")
}

pub struct Skrivare {
    db: Connection,
    max: usize,
}

impl Skrivare {
    /// Skapar en ny, tom databas (en gammal fil med samma namn tas bort: ny körning).
    pub fn ny(vag: &Path, fast: &Fast) -> Result<Skrivare, String> {
        Skrivare::ny_med_tak(vag, fast, MAX_LAGEN)
    }

    pub fn ny_med_tak(vag: &Path, fast: &Fast, max: usize) -> Result<Skrivare, String> {
        assert!(max >= 4, "taket ska vara minst 4");
        assert!(fast.b <= 256 && fast.h <= 128, "kartan ryms inte i djurens två byte");
        assert_eq!(fast.vmax.len(), fast.b * fast.h);
        assert_eq!(fast.vatten.len(), fast.b * fast.h);
        for sv in ["", "-wal", "-shm"] {
            let mut p = vag.as_os_str().to_owned();
            p.push(sv);
            let _ = std::fs::remove_file(PathBuf::from(p));
        }
        let db = Connection::open(vag).map_err(fel("öppna"))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
             CREATE TABLE fast (rubrik TEXT NOT NULL, b INTEGER NOT NULL, h INTEGER NOT NULL, steg_per_ar INTEGER NOT NULL,
                                vmax BLOB NOT NULL, vatten BLOB NOT NULL);
             CREATE TABLE lage (id INTEGER PRIMARY KEY AUTOINCREMENT, klocka INTEGER NOT NULL, t INTEGER NOT NULL,
                                markerad INTEGER NOT NULL, v BLOB NOT NULL, djur BLOB NOT NULL);",
        )
        .map_err(fel("skapa"))?;
        db.execute(
            "INSERT INTO fast (rubrik, b, h, steg_per_ar, vmax, vatten) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![fast.rubrik, fast.b as i64, fast.h as i64, fast.steg_per_ar, f32_till_blob(&fast.vmax), fast.vatten],
        )
        .map_err(fel("skriv fast"))?;
        Ok(Skrivare { db, max })
    }

    /// Lägger till ett läge. Ger `true` när databasen just har glesats ut; då ska tiden mellan lägena dubblas.
    pub fn lagg_till(&mut self, l: &Lage) -> Result<bool, String> {
        self.db
            .execute(
                "INSERT INTO lage (klocka, t, markerad, v, djur) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![l.klocka as i64, l.t, l.markerad as i64, f32_till_blob(&l.v), djur_till_blob(&l.djur)],
            )
            .map_err(fel("skriv läge"))?;
        let n: i64 = self.db.query_row("SELECT COUNT(*) FROM lage", [], |r| r.get(0)).map_err(fel("räkna"))?;
        if (n as usize) <= self.max {
            return Ok(false);
        }
        // Gles ut: behåll vartannat läge räknat från det första, och alltid det sista.
        let ids: Vec<i64> = {
            let mut q = self.db.prepare("SELECT id FROM lage ORDER BY id").map_err(fel("lista"))?;
            let r = q.query_map([], |r| r.get(0)).map_err(fel("lista"))?;
            r.collect::<Result<_, _>>().map_err(fel("lista"))?
        };
        let sista = ids.len() - 1;
        let tx = self.db.transaction().map_err(fel("glesa"))?;
        for (k, id) in ids.iter().enumerate() {
            if k % 2 == 1 && k != sista {
                tx.execute("DELETE FROM lage WHERE id = ?1", params![id]).map_err(fel("glesa"))?;
            }
        }
        tx.commit().map_err(fel("glesa"))?;
        let _ = self.db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;");
        Ok(true)
    }
}

pub struct Lasare {
    db: Connection,
}

impl Lasare {
    /// Öppnar databasen bara för läsning; `None` om filen saknas eller inte går att läsa än.
    pub fn oppna(vag: &Path) -> Option<Lasare> {
        if !vag.exists() {
            return None;
        }
        let db = Connection::open_with_flags(vag, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
        let _ = db.busy_timeout(std::time::Duration::from_millis(500));
        db.query_row("SELECT COUNT(*) FROM fast", [], |r| r.get::<_, i64>(0)).ok()?;
        Some(Lasare { db })
    }

    pub fn fast(&self) -> Result<Fast, String> {
        self.db
            .query_row("SELECT rubrik, b, h, steg_per_ar, vmax, vatten FROM fast", [], |r| {
                Ok(Fast {
                    rubrik: r.get(0)?,
                    b: r.get::<_, i64>(1)? as usize,
                    h: r.get::<_, i64>(2)? as usize,
                    steg_per_ar: r.get(3)?,
                    vmax: blob_till_f32(&r.get::<_, Vec<u8>>(4)?),
                    vatten: r.get(5)?,
                })
            })
            .map_err(fel("läs fast"))
    }

    /// (id, klocka, steg) för alla lägen, äldst först.
    pub fn lista(&self) -> Result<Vec<(i64, u64, i64)>, String> {
        let mut q = self.db.prepare("SELECT id, klocka, t FROM lage ORDER BY id").map_err(fel("lista"))?;
        let r = q
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as u64, r.get::<_, i64>(2)?)))
            .map_err(fel("lista"))?;
        r.collect::<Result<_, _>>().map_err(fel("lista"))
    }

    pub fn lage(&self, id: i64) -> Result<Lage, String> {
        self.db
            .query_row("SELECT klocka, t, markerad, v, djur FROM lage WHERE id = ?1", params![id], |r| {
                Ok(Lage {
                    klocka: r.get::<_, i64>(0)? as u64,
                    t: r.get(1)?,
                    markerad: r.get::<_, i64>(2)? != 0,
                    v: blob_till_f32(&r.get::<_, Vec<u8>>(3)?),
                    djur: blob_till_djur(&r.get::<_, Vec<u8>>(4)?),
                })
            })
            .map_err(fel("läs läge"))
    }

    /// Läget som samma JSON-text som kartbildens fil (med `klocka` först), så att kartvyn läser det som förut.
    pub fn lage_json(&self, id: i64) -> Result<String, String> {
        let f = self.fast()?;
        let l = self.lage(id)?;
        Ok(lage_json(&f, &l))
    }
}

pub fn lage_json(f: &Fast, l: &Lage) -> String {
    let v: Vec<String> = l.v.iter().map(|x| format!("{x:.3}")).collect();
    let tak: Vec<String> = f.vmax.iter().map(|x| format!("{x:.3}")).collect();
    let vatten: Vec<String> = f.vatten.iter().map(|x| x.to_string()).collect();
    let djur: Vec<String> = l.djur.iter().map(|&(i, j, m)| format!("[{},{},{}]", i, j, if m { 1 } else { 0 })).collect();
    format!(
        "{{\"klocka\":{},\"rubrik\":{},\"t\":{},\"ar\":{:.2},\"markerad\":{},\"b\":{},\"h\":{},\"v\":[{}],\"vmax\":[{}],\"vatten\":[{}],\"djur\":[{}]}}",
        l.klocka,
        serde_json::to_string(&f.rubrik).unwrap_or_else(|_| "\"\"".into()),
        l.t,
        l.t as f64 / f.steg_per_ar as f64,
        l.markerad,
        f.b,
        f.h,
        v.join(","),
        tak.join(","),
        vatten.join(","),
        djur.join(",")
    )
}
