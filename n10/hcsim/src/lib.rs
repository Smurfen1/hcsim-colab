//! hcsim: simulering på Hell Creek-kartan (S5a växter/klimat, S5b Edmontosaurus med hjärna).
//! Kartan läses bara (Alt 1199). `n10/varld/` rörs inte.

pub mod avlasare;
pub mod bitbin;
pub mod diagnos;
pub mod formaga;
pub mod hjarna;
pub mod hjarna2008;
pub mod hjarnab1;
pub mod hjarnprov;
pub mod hjarnstorlek;
pub mod hjsignal;
pub mod kartbild;
pub mod klimat;
pub mod kopier;
pub mod ledprov;
pub mod minstakvadrat;
pub mod rang;
pub mod rng;
pub mod serdej;
pub mod signal;
pub mod sim;
pub mod spara;
pub mod svep;
pub mod tal;
pub mod tidsskift;
pub mod tur;
pub mod utforsk;
pub mod vagsamlare;
pub mod vmodell;

pub use formaga::{Formaga, HjarnArm, Intagslasare, Minnestabell};
pub use hjarna::Hj;
pub use klimat::Klimat;
pub use rng::Rng;
pub use sim::{
    sammanfattning_json, AldrigAck, ArRad, Bokforing, BokforingBlock, EpsArAck, FormagaMatning,
    KopierArAck, KopierMatning, LedLogg, LedLoggRad, MarkLage, Matning, SignalLogg, SignalLoggRad,
    Sim, UtforskMatning, MARKT_FRO_XOR,
};
pub use spara::{
    kor_fler_ar, kor_med_spara, kor_till_ar, ladda, spara, spara_utan_g_PROV, KorParametrar,
    SPAR_VERSION,
};
